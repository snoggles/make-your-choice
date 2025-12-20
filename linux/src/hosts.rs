use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::process::Command;
use crate::region::{BlockMode, RegionInfo, get_group_name};

const SECTION_MARKER: &str = "# --+ Make Your Choice +--";
const HOSTS_PATH: &str = "/etc/hosts";

pub struct HostsManager {
    discord_url: String,
}

impl HostsManager {
    pub fn new(discord_url: String) -> Self {
        Self { discord_url }
    }

    fn read_hosts(&self) -> Result<String> {
        fs::read_to_string(HOSTS_PATH)
            .or_else(|_| Ok(String::new()))
    }

    fn write_hosts(&self, content: &str) -> Result<()> {
        // Write to a temporary file first
        let temp_path = "/tmp/make-your-choice-hosts.tmp";
        fs::write(temp_path, content)
            .context("Failed to write temporary file")?;

        // Combine all operations into a single pkexec call to avoid multiple prompts
        let combined_command = format!(
            "cp {} {}.bak 2>/dev/null || true && cp {} {} && (systemd-resolve --flush-caches 2>/dev/null || resolvectl flush-caches 2>/dev/null || nscd -i hosts 2>/dev/null || true)",
            HOSTS_PATH, HOSTS_PATH, temp_path, HOSTS_PATH
        );

        let status = Command::new("pkexec")
            .arg("sh")
            .arg("-c")
            .arg(&combined_command)
            .status()
            .context("Failed to execute pkexec")?;

        // Clean up temp file
        let _ = fs::remove_file(temp_path);

        if !status.success() {
            bail!("Failed to write to {}. Operation was cancelled or permission was denied.", HOSTS_PATH);
        }

        Ok(())
    }

    fn write_wrapped_section(&self, inner_content: &str) -> Result<()> {
        let original = self.read_hosts()?;

        // Find existing markers
        let first = original.find(SECTION_MARKER);
        let last = if let Some(pos) = first {
            original[pos + SECTION_MARKER.len()..].find(SECTION_MARKER)
                .map(|p| p + pos + SECTION_MARKER.len())
        } else {
            None
        };

        // Build new wrapped block
        let wrapped = if inner_content.is_empty() {
            String::new()
        } else {
            let mut content = inner_content.to_string();
            if !content.ends_with('\n') {
                content.push('\n');
            }
            format!("{}\n{}{}\n", SECTION_MARKER, content, SECTION_MARKER)
        };

        let new_content = match (first, last) {
            (Some(f), Some(l)) => {
                // Replace everything between markers
                format!("{}{}{}", &original[..f], wrapped, &original[l + SECTION_MARKER.len()..])
            }
            (Some(f), None) => {
                // Corrupt state: replace from first marker to end
                format!("{}{}", &original[..f], wrapped)
            }
            (None, _) => {
                // No markers: append
                let suffix = if original.ends_with('\n') { "\n" } else { "\n\n" };
                format!("{}{}{}", original, suffix, wrapped)
            }
        };

        self.write_hosts(&new_content)
    }

    pub fn apply_gatekeep(
        &self,
        regions: &HashMap<String, RegionInfo>,
        selected: &HashSet<String>,
        block_mode: BlockMode,
        merge_unstable: bool,
    ) -> Result<()> {
        if selected.is_empty() {
            bail!("Please select at least one server to allow.");
        }

        // Check if any stable servers are selected
        let any_stable_selected = selected.iter()
            .any(|r| regions.get(r).map(|info| info.stable).unwrap_or(false));

        // Merge unstable servers with stable alternatives if needed
        let mut allowed_set = selected.clone();
        if merge_unstable && !any_stable_selected {
            for region in selected.iter() {
                if let Some(info) = regions.get(region) {
                    if !info.stable {
                        let group = get_group_name(region);
                        // Find a stable alternative in the same group
                        if let Some((alt_region, _)) = regions.iter()
                            .find(|(r, i)| get_group_name(r) == group && i.stable)
                        {
                            allowed_set.insert(alt_region.clone());
                        }
                    }
                }
            }
        }

        // Build hosts content
        let mut content = String::new();
        content.push_str("# Edited by Make Your Choice (DbD Server Selector)\n");
        content.push_str("# Unselected servers are blocked (Gatekeep Mode); selected servers are commented out.\n");
        content.push_str(&format!("# Need help? Discord: {}\n", self.discord_url));
        content.push_str("\n");

        for (region_key, region_info) in regions.iter() {
            let allow = allowed_set.contains(region_key);
            for host in &region_info.hosts {
                let is_ping = host.to_lowercase().contains("ping");
                let include = match block_mode {
                    BlockMode::Both => true,
                    BlockMode::OnlyPing => is_ping,
                    BlockMode::OnlyService => !is_ping,
                };

                if include {
                    let prefix = if allow { "#" } else { "0.0.0.0" };
                    content.push_str(&format!("{:9} {}\n", prefix, host));
                }
            }
            content.push_str("\n");
        }

        self.write_wrapped_section(&content)?;
        Ok(())
    }

    pub fn apply_universal_redirect(
        &self,
        regions: &HashMap<String, RegionInfo>,
        selected_region: &str,
    ) -> Result<()> {
        let region_info = regions.get(selected_region)
            .context("Selected region not found")?;

        let service_host = &region_info.hosts[0];
        let ping_host = if region_info.hosts.len() > 1 {
            &region_info.hosts[1]
        } else {
            &region_info.hosts[0]
        };

        // Resolve IP addresses
        let service_ip = resolve_hostname(service_host)?;
        let ping_ip = resolve_hostname(ping_host)?;

        // Build hosts content
        let mut content = String::new();
        content.push_str("# Edited by Make Your Choice (DbD Server Selector)\n");
        content.push_str("# Universal Redirect mode: redirect all GameLift endpoints to selected region\n");
        content.push_str(&format!("# Need help? Discord: {}\n", self.discord_url));
        content.push_str("\n");

        for (_, region_info) in regions.iter() {
            for host in &region_info.hosts {
                let is_ping = host.to_lowercase().contains("ping");
                let ip = if is_ping { &ping_ip } else { &service_ip };
                content.push_str(&format!("{} {}\n", ip, host));
            }
            content.push_str("\n");
        }

        self.write_wrapped_section(&content)?;
        Ok(())
    }

    pub fn revert(&self) -> Result<()> {
        self.write_wrapped_section("")?;
        Ok(())
    }

    pub fn restore_default(&self) -> Result<()> {
        let default_hosts = "# Static table lookup for hostnames.
# See hosts(5) for details.
127.0.0.1        localhost
::1              localhost
";

        self.write_hosts(default_hosts)?;
        Ok(())
    }
}

fn resolve_hostname(hostname: &str) -> Result<String> {
    use std::net::ToSocketAddrs;

    let addr = format!("{}:443", hostname)
        .to_socket_addrs()
        .with_context(|| format!("Failed to resolve hostname: {}", hostname))?
        .next()
        .context("No addresses found")?;

    Ok(addr.ip().to_string())
}
