use tokio::time::{delay_for, Duration};

#[derive(Clone)]
pub struct Nixhub {
    pub repo: String,
    pub reference: String,
    pub out: String,
    pub token: Option<String>,
}

impl Nixhub {
    pub async fn request_tarball_location(
        &self,
    ) -> Option<reqwest::header::HeaderValue> {
        let url = format!(
            "https://api.github.com/repos/{}/tarball/{}",
            self.repo, self.reference
        );
        let client = reqwest::Client::builder()
            .user_agent("Nix builder")
            .redirect(reqwest::redirect::Policy::none())
            .build().ok()?;
        let mut request = client.get(&url);
        if let Some(token) = self.token.as_ref() {
            request = request.header("Authorization", format!("token {}", token));
        }
        request.send().await.ok().and_then(|resp: reqwest::Response| {
            resp.headers().get("location").map(|l| l.clone())
        })
    }

    pub async fn build(&self) -> Result<(), std::io::Error> {
        let mut dt = 200;
        loop {
            if let Some(location) = self.request_tarball_location().await {
                let mut cmd = std::process::Command::new("nix");
                cmd.arg("build")
                    .arg("--out-link")
                    .arg(&self.out)
                    .arg("-f")
                    .arg(location.to_str().unwrap());
                if cmd.status()?.success() {
                    eprintln!("Built \"{}\" at ref \"{}\" successfully", self.repo, self.reference);
                    break;
                } else {
                    eprintln!("Failed to execute {:?}", cmd);
                }
            } else {
                eprintln!("Failed to get tarball location for \"{}\" at ref \"{}\"", self.repo, self.reference);
                delay_for(Duration::from_millis(dt)).await;
                // exponential decay
                if dt < 30 * 60000 {
                    dt = dt * 2;
                }
            }
        }
        Ok(())
    }
}
