use std::{path::PathBuf, str::FromStr};

use clap::{Parser, Subcommand};
use notion::ids::PageId;
use reqwest::Url;
use thiserror::Error;
use url::Host;
use uuid::Uuid;

#[derive(Parser)]
#[clap(author, version, about)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Fetch {
        #[clap(short = 'p', long = "page-id", value_parser = page_id_parser)]
        page_id: PageId,

        #[clap(short = 'f', long = "file")]
        file: PathBuf,
    },
    Push {
        #[clap(short = 'p', long = "page-id", value_parser = page_id_parser)]
        page_id: PageId,

        #[clap(short = 'f', long = "file")]
        file: PathBuf,
    },
}

fn page_id_parser(s: &str) -> Result<PageId, String> {
    parse_page_id(s).map_err(|error| error.to_string())
}

#[derive(Error, Debug, PartialEq, Eq)]
enum ParsePageIdError {
    #[error("Invalid URL: {0}")]
    Url(#[from] ParsePageIdFromUrlError),

    #[error("Invalid UUID: {0}")]
    Uuid(#[from] ParsePageIdFromUuidError),
}

fn parse_page_id(s: &str) -> Result<PageId, ParsePageIdError> {
    if let Ok(url) = Url::parse(s) {
        parse_page_id_from_url(url).map_err(Into::into)
    } else {
        parse_page_id_from_uuid(s).map_err(Into::into)
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
enum ParsePageIdFromUrlError {
    #[error("Missing hostname")]
    MissingHostname,

    #[error("Not a Notion URL: {0}")]
    NotNotionHostname(Host<String>),

    #[error("Empty path")]
    NoPathSegments,

    #[error("Page ID missing in the URL. Expected page ID to be the 2nd segment in the path")]
    NotEnoughPathSegments,

    #[error("Invalid UUID in path. {0}")]
    InvalidUuidInPath(#[source] ParsePageIdFromUuidError),

    #[error("Invalid UUID {uuid_candidate} in query parameters. {source}")]
    InvalidUuidInQuery {
        #[source]
        source: ParsePageIdFromUuidError,

        uuid_candidate: String,
    },
}

fn parse_page_id_from_url<'a>(url: Url) -> Result<PageId, ParsePageIdFromUrlError> {
    match url.host() {
        None => return Err(ParsePageIdFromUrlError::MissingHostname),
        Some(Host::Domain("www.notion.so")) => {}
        Some(host) => return Err(ParsePageIdFromUrlError::NotNotionHostname(host.to_owned())),
    };

    let second_segment = match url.path_segments() {
        None => Err(ParsePageIdFromUrlError::NoPathSegments),
        Some(mut path_segments) => path_segments
            .nth(1)
            .ok_or(ParsePageIdFromUrlError::NotEnoughPathSegments),
    }?;

    let path_uuid_candidate = second_segment
        .split('-')
        .last()
        .ok_or(ParsePageIdFromUrlError::NotEnoughPathSegments)?;

    let path_uuid = parse_page_id_from_uuid(path_uuid_candidate)
        .map_err(ParsePageIdFromUrlError::InvalidUuidInPath)?;

    match url.query_pairs().find(|(key, _value)| key == "p") {
        None => Ok(path_uuid),
        Some((_key, query_uuid_candidate)) => parse_page_id_from_uuid(&query_uuid_candidate)
            .map_err(|error| ParsePageIdFromUrlError::InvalidUuidInQuery {
                source: error,
                uuid_candidate: query_uuid_candidate.to_string(),
            }),
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
enum ParsePageIdFromUuidError {
    #[error("Cannot parse UUID: {0}")]
    CannotParse(#[from] uuid::Error),
}

fn parse_page_id_from_uuid(s: &str) -> Result<PageId, ParsePageIdFromUuidError> {
    let parsed_uuid = Uuid::try_parse(s).map_err(ParsePageIdFromUuidError::CannotParse)?;

    Ok(PageId::from_str(&parsed_uuid.hyphenated().to_string())
        .expect("notion crate PageId does not do any validation when parsing"))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn uuid_from_valid_notion_urls() {
        {
            let url_with_hash = "https://www.notion.so/gregorr/Test-page-0b89a6e8f0064acc8ec6e6902b039e3a#951424222c544b4896fc599a043f0c3d";
            assert_eq!(
                parse_page_id(url_with_hash).unwrap().to_string(),
                "0b89a6e8-f006-4acc-8ec6-e6902b039e3a"
            );
        }

        {
            let url_without_hash =
                "https://www.notion.so/gregorr/Test-page-0b89a6e8f0064acc8ec6e6902b039e3a";
            assert_eq!(
                parse_page_id(url_without_hash).unwrap().to_string(),
                "0b89a6e8-f006-4acc-8ec6-e6902b039e3a"
            );
        }

        {
            let url_without_page_name =
                "https://www.notion.so/gregorr/0b89a6e8f0064acc8ec6e6902b039e3a";
            assert_eq!(
                parse_page_id(url_without_page_name).unwrap().to_string(),
                "0b89a6e8-f006-4acc-8ec6-e6902b039e3a"
            );
        }

        {
            let database_url_with_center_peek = "https://www.notion.so/gregorr/7659d7a185384403a1d603b828a21561?v=1156113b60bd45a48187e2fb5448f5ec&p=0b89a6e8f0064acc8ec6e6902b039e3a&pm=c";
            assert_eq!(
                parse_page_id(database_url_with_center_peek)
                    .unwrap()
                    .to_string(),
                "0b89a6e8-f006-4acc-8ec6-e6902b039e3a"
            );
        }
    }

    #[test]
    fn invalid_notion_urls() {
        {
            let github_url = "https://github.com/";
            assert_eq!(
                parse_page_id(github_url),
                Err(ParsePageIdError::Url(
                    ParsePageIdFromUrlError::NotNotionHostname(Host::Domain(
                        "github.com".to_string()
                    ))
                ))
            );
        }

        {
            let notion_url_without_page = "https://www.notion.so/";
            assert_eq!(
                parse_page_id(notion_url_without_page),
                Err(ParsePageIdError::Url(
                    ParsePageIdFromUrlError::NotEnoughPathSegments
                ))
            );
        }
    }

    #[test]
    fn uuid_from_string() {
        assert_eq!(
            parse_page_id("0b89a6e8f0064acc8ec6e6902b039e3a")
                .unwrap()
                .to_string(),
            "0b89a6e8-f006-4acc-8ec6-e6902b039e3a"
        );

        assert_eq!(
            parse_page_id("0b89a6e8-f006-4acc-8ec6-e6902b039e3a")
                .unwrap()
                .to_string(),
            "0b89a6e8-f006-4acc-8ec6-e6902b039e3a"
        );
    }

    #[test]
    fn invalid_uuids() {
        assert!(matches!(
            parse_page_id("some invalid uuid"),
            Err(ParsePageIdError::Uuid(
                ParsePageIdFromUuidError::CannotParse(_)
            ))
        ));
    }
}
