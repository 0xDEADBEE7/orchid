use super::{ClientError, ClientErrorKind};

#[derive(Clone, Copy)]
pub(super) enum Api {
    Chat,
    Responses,
}

pub(super) fn resolve(base: &str, api: Api) -> Result<String, ClientError> {
    let mut url = reqwest::Url::parse(base).map_err(|_| configuration_error("invalid base URL"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(configuration_error("base URL must use HTTP or HTTPS"));
    }
    let path = url.path().trim_end_matches('/');
    let expected = match api {
        Api::Chat => "/chat/completions",
        Api::Responses => "/responses",
    };
    if path.ends_with(expected) {
        return Ok(base.into());
    }
    if ["/chat/completions", "/responses", "/messages"]
        .iter()
        .any(|endpoint| path.ends_with(endpoint))
    {
        return Err(configuration_error(
            "endpoint does not match the configured client interface",
        ));
    }
    let suffix = match api {
        Api::Chat if !path.ends_with("/v1") => "/v1/chat/completions",
        _ => expected,
    };
    url.set_path(&format!("{path}{suffix}"));
    Ok(url.into())
}

fn configuration_error(message: &str) -> ClientError {
    ClientError::new(ClientErrorKind::Configuration, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_complete_endpoints() {
        for (api, path) in [
            (Api::Chat, "chat/completions"),
            (Api::Responses, "responses"),
        ] {
            for suffix in ["", "/", "?api-version=2024-10-21", "/?x=a%2Fb#section"] {
                let url = format!("https://example.com/custom/v1/{path}{suffix}");
                assert_eq!(resolve(&url, api).unwrap(), url);
            }
        }
    }

    #[test]
    fn appends_only_to_the_path() {
        for (api, base, expected) in [
            (
                Api::Chat,
                "https://example.com",
                "https://example.com/v1/chat/completions",
            ),
            (
                Api::Chat,
                "https://example.com/v1/?x=1",
                "https://example.com/v1/chat/completions?x=1",
            ),
            (
                Api::Chat,
                "https://example.com/proxy",
                "https://example.com/proxy/v1/chat/completions",
            ),
            (
                Api::Responses,
                "https://example.com/v1/?x=1",
                "https://example.com/v1/responses?x=1",
            ),
            (
                Api::Responses,
                "https://example.com/backend-api/codex",
                "https://example.com/backend-api/codex/responses",
            ),
            (
                Api::Chat,
                "https://example.com?next=/chat/completions",
                "https://example.com/v1/chat/completions?next=/chat/completions",
            ),
        ] {
            assert_eq!(resolve(base, api).unwrap(), expected);
        }
    }

    #[test]
    fn rejects_incompatible_endpoints_and_invalid_urls() {
        for (api, url) in [
            (Api::Chat, "https://example.com/v1/responses/?x=1"),
            (Api::Responses, "https://example.com/v1/chat/completions"),
            (Api::Chat, "https://example.com/v1/messages"),
            (Api::Chat, "not a URL"),
            (Api::Responses, "file:///v1/responses"),
        ] {
            assert!(resolve(url, api).is_err());
        }
    }
}
