use curl::easy::{Easy, List};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::time::Duration;
use std::io::{Error, ErrorKind};

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
    pub fn request_tarball_location(&self) -> Result<Option<String>, Error> {
        let mut easy = Easy::new();
        easy.useragent("Deplorable")?;
        if let Some(token) = self.token.as_ref() {
            let mut headers = List::new();
            headers.append(format!("Authorization: token {}", token).as_str())?;
            easy.http_headers(headers)?;
        }

        #[derive(Deserialize)]
        struct Commit {
            pub sha: String,
        }

        let commit: Commit = {
            easy.url(format!("https://api.github.com/repos/{}/commits/{}",
                    self.repo, self.reference).as_str())?;
            let mut body = Vec::new();
            {
                let mut transfer = easy.transfer();
                transfer.write_function(|data| {
                    body.extend_from_slice(data);
                    Ok(data.len())
                })?;
                transfer.perform()?;
            }
            match easy.response_code()? {
                200 => serde_yaml::from_slice(body.as_ref()).map_err(|e| Error::new(ErrorKind::Other, e))?,
                code => {
                    return Err(Error::new(ErrorKind::InvalidInput, format!("Repository not accessible ({})", code)));
                }
            }
        };

        let url = format!(
            "https://api.github.com/repos/{}/tarball/{}",
            self.repo, commit.sha
        );
        easy.url(url.as_str())?;
        easy.transfer().perform()?;
        Ok(easy.redirect_url()?.map(|s| s.to_string()))
    }

    pub fn build(&self) -> Result<(), std::io::Error> {
        let mut dt = 200;
        loop {
            if let Some(location) = self.request_tarball_location()? {
                let mut cmd = std::process::Command::new("nix");
                cmd.arg("build")
                    .arg("--out-link")
                    .arg(&self.out)
                    .arg("-f")
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
