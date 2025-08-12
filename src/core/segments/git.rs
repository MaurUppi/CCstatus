use super::Segment;
use crate::config::InputData;
use std::process::Command;

#[derive(Debug)]
pub struct GitInfo {
    pub branch: String,
    pub status: GitStatus,
    pub ahead: u32,
    pub behind: u32,
    pub sha: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum GitStatus {
    Clean,
    Dirty,
    Conflicts,
}

pub struct GitSegment {
    enabled: bool,
    show_sha: bool,
}

impl GitSegment {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            show_sha: false,
        }
    }

    pub fn with_sha(mut self, show_sha: bool) -> Self {
        self.show_sha = show_sha;
        self
    }

    fn get_git_info(&self, working_dir: &str) -> Option<GitInfo> {
        // First check if we're in a Git repository
        if !self.is_git_repository(working_dir) {
            return None;
        }

        let branch = self
            .get_branch(working_dir)
            .unwrap_or_else(|| "detached".to_string());
        let status = self.get_status(working_dir);
        let (ahead, behind) = self.get_ahead_behind(working_dir);
        let sha = if self.show_sha {
            self.get_sha(working_dir)
        } else {
            None
        };

        Some(GitInfo {
            branch,
            status,
            ahead,
            behind,
            sha,
        })
    }

    fn is_git_repository(&self, working_dir: &str) -> bool {
        Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(working_dir)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn get_branch(&self, working_dir: &str) -> Option<String> {
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(working_dir)
            .output()
            .ok()?;

        if output.status.success() {
            let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
            if branch.is_empty() {
                None
            } else {
                Some(branch)
            }
        } else {
            None
        }
    }

    fn get_status(&self, working_dir: &str) -> GitStatus {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(working_dir)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let status_text = String::from_utf8(output.stdout).unwrap_or_default();

                if status_text.trim().is_empty() {
                    return GitStatus::Clean;
                }

                // Check for merge conflict markers
                if status_text.contains("UU")
                    || status_text.contains("AA")
                    || status_text.contains("DD")
                {
                    GitStatus::Conflicts
                } else {
                    GitStatus::Dirty
                }
            }
            _ => GitStatus::Clean,
        }
    }

    fn get_ahead_behind(&self, working_dir: &str) -> (u32, u32) {
        let ahead = self.get_commit_count(working_dir, "@{u}..HEAD");
        let behind = self.get_commit_count(working_dir, "HEAD..@{u}");
        (ahead, behind)
    }

    fn get_commit_count(&self, working_dir: &str, range: &str) -> u32 {
        let output = Command::new("git")
            .args(["rev-list", "--count", range])
            .current_dir(working_dir)
            .output();

        match output {
            Ok(output) if output.status.success() => String::from_utf8(output.stdout)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0),
            _ => 0,
        }
    }

    fn get_sha(&self, working_dir: &str) -> Option<String> {
        let output = Command::new("git")
            .args(["rev-parse", "--short=7", "HEAD"])
            .current_dir(working_dir)
            .output()
            .ok()?;

        if output.status.success() {
            let sha = String::from_utf8(output.stdout).ok()?.trim().to_string();
            if sha.is_empty() {
                None
            } else {
                Some(sha)
            }
        } else {
            None
        }
    }

    fn format_git_status(&self, info: &GitInfo) -> String {
        let mut parts = Vec::new();

        // Branch name with Nerd Font branch icon
        parts.push(format!("\u{f02a2} {}", info.branch));

        // Status indicators using simple Unicode symbols
        match info.status {
            GitStatus::Clean => parts.push("✓".to_string()),
            GitStatus::Dirty => parts.push("●".to_string()),
            GitStatus::Conflicts => parts.push("⚠".to_string()),
        }

        // Remote tracking status with arrows
        if info.ahead > 0 {
            parts.push(format!("↑{}", info.ahead));
        }
        if info.behind > 0 {
            parts.push(format!("↓{}", info.behind));
        }

        // Short SHA hash
        if let Some(ref sha) = info.sha {
            parts.push(sha.clone());
        }

        parts.join(" ")
    }
}

impl Segment for GitSegment {
    fn render(&self, input: &InputData) -> String {
        if !self.enabled {
            return String::new();
        }

        match self.get_git_info(&input.workspace.current_dir) {
            Some(git_info) => self.format_git_status(&git_info),
            None => String::new(), // Not in a Git repository
        }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }
}
