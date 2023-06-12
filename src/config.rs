use reqwest::redirect::Policy;
use reqwest::{header, blocking::Client};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub repos: BTreeMap<String, Repo>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Repo {
    pub repo: String,
    pub reference: String,
    pub out: String,
    pub token: Option<String>,
    pub secret: Option<String>,
}

impl Repo {
    pub fn request_tarball_location(&self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let mut builder = Client::builder().user_agent("Deplorable").redirect(Policy::none());
        if let Some(token) = self.token.as_ref() {
            let mut headers = header::HeaderMap::new();
            headers.insert(header::AUTHORIZATION, header::HeaderValue::from_str(format!("token {}", token).as_str())?);
            builder = builder.default_headers(headers);
        }

        let client = builder.build()?;

        let commit = {
            let url = format!(
                "https://api.github.com/repos/{}/branches/{}",
                self.repo, self.reference
            );
            #[derive(Deserialize)]
            struct Commit {
                sha: String,
            }
            #[derive(Deserialize)]
            struct Branch {
                commit: Commit,
            }
            let branch: Branch = client.get(url).send()?.json()?;
            branch.commit.sha
        };

        {
            let url = format!(
                "https://api.github.com/repos/{}/tarball/{}",
                self.repo, commit
            );

            let location = client.get(url).send()?.headers().get(header::LOCATION).and_then(|s| s.to_str().ok()).map(String::from);
            Ok(location)
        }
    }

    pub fn build(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut dt = 200;
        loop {
            if let Some(location) = self.request_tarball_location()? {
                let mut cmd = std::process::Command::new("nix-build");
                cmd.arg("--out-link")
                    .arg(&self.out)
                    .arg(location);
                if cmd.status()?.success() {
                    eprintln!(
                        "Successfully built \"{}\" at ref \"{}\"",
                        self.repo, self.reference
                    );
                    break;
                } else {
                    eprintln!("Failed to execute {:?}", cmd);
                }
            } else {
                eprintln!(
                    "Failed to get tarball location for \"{}\" at ref \"{}\"",
                    self.repo, self.reference
                );
                std::thread::sleep(Duration::from_millis(dt));
                // exponential decay
                if dt < 30 * 60000 {
                    dt = dt * 2;
                }
            }
        }
        Ok(())
    }
}
