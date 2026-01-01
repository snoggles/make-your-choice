use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PatchNotes {
    version: String,
    notes: Vec<String>,
}

pub fn load_versinf() -> (String, String) {
    const VERSINF_YAML: &str = include_str!("../../VERSINF.yaml");

    match serde_yaml::from_str::<PatchNotes>(VERSINF_YAML) {
        Ok(versinf) => {
            let version = versinf.version;
            let notes = versinf.notes
                .iter()
                .map(|note| format!("- {}", note))
                .collect::<Vec<_>>()
                .join("\n");

            let message = format!("Here are some new features and changes:\n\n{}", notes);
            (version, message)
        }
        Err(_) => {
            ("v0.0.0".to_string(), "Failed to get version info.".to_string())
        }
    }
}

pub async fn fetch_git_identity() -> Option<String> {
    const UID: &str = "109703063"; // Changing this, or the final result of this functionality may break license compliance
    let url = format!("https://api.github.com/user/{}", UID);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    match client
        .get(&url)
        .header("User-Agent", "make-your-choice")
        .send()
        .await
    {
        Ok(response) => {
            if let Ok(json) = response.json::<serde_json::Value>().await {
                if let Some(login) = json.get("login").and_then(|v| v.as_str()) {
                    return Some(login.to_string());
                }
            }
        }
        Err(_) => {}
    }

    None
}
