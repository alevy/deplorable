use curl::easy::{Easy, List};
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
    pub fn request_tarball_location(&self) -> Result<Option<String>, std::io::Error> {
        let url = format!(
            "https://api.github.com/repos/{}/tarball/{}",
            self.repo, self.reference
        );
        let mut easy = Easy::new();
        easy.url(url.as_str())?;
        easy.useragent("Deplorable")?;
        if let Some(token) = self.token.as_ref() {
            let mut headers = List::new();
            headers.append(format!("Authorization: token {}", token).as_str())?;
            easy.http_headers(headers)?;
        }
        easy.perform()?;
        Ok(easy.redirect_url()?.map(|s| s.to_string()))
    }

    pub fn build(&self) -> Result<(), std::io::Error> {
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
