use anyhow::Result;
use duct::cmd;
use indicatif::ProgressBar;

pub struct CommandRunner {
    progress_bar: Option<ProgressBar>,
}

impl CommandRunner {
    pub fn new() -> Self {
        Self {
            progress_bar: None,
        }
    }

    pub async fn run_command(&mut self, command: String, args: Vec<String>) -> Result<String> {
        let message = format!("Running: {} {}", command, args.join(" "));
        let pb = ProgressBar::new_spinner();
        pb.set_message(message.clone());
        self.progress_bar = Some(pb);

        let result = tokio::task::spawn_blocking(move || {
            let expression = cmd(command, args);
            expression.read()
        }).await;

        if let Some(pb) = &mut self.progress_bar {
            pb.finish();
        }

        match result {
            Ok(output) => {
                match output {
                    Ok(stdout) => Ok(stdout),
                    Err(e) => Err(anyhow::anyhow!("Command failed: {}", e))
                }
            }
            Err(e) => Err(anyhow::anyhow!("Failed to run command: {}", e))
        }
    }

    pub async fn run_command_with_progress(&mut self, command: String, args: Vec<String>, message: String) -> Result<String> {
        let pb = ProgressBar::new_spinner();
        pb.set_message(message.clone());
        self.progress_bar = Some(pb);

        let result = tokio::task::spawn_blocking(move || {
            let expression = cmd(command, args);
            expression.read()
        }).await;

        if let Some(pb) = &mut self.progress_bar {
            pb.finish();
        }

        match result {
            Ok(output) => {
                match output {
                    Ok(stdout) => Ok(stdout),
                    Err(e) => Err(anyhow::anyhow!("Command failed: {}", e))
                }
            }
            Err(e) => Err(anyhow::anyhow!("Failed to run command: {}", e))
        }
    }

    pub fn run_sync_command(&mut self, command: &str, args: Vec<&str>) -> Result<String> {
        let cmd_args: Vec<String> = args.iter().map(|&s| s.to_string()).collect();
        let expression = cmd(command, cmd_args);
        let output = expression.read()?;

        Ok(output)
    }
}

impl Drop for CommandRunner {
    fn drop(&mut self) {
        if let Some(pb) = &mut self.progress_bar {
            pb.finish();
        }
    }
}