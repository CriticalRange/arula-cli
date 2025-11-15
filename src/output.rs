use std::io::{self, Write};
use console::style;

pub struct OutputHandler {
    debug: bool,
}

impl OutputHandler {
    pub fn new() -> Self {
        Self { debug: false }
    }

    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    pub fn is_debug(&self) -> bool {
        self.debug
    }

    pub fn print_user_message(&mut self, content: &str) -> io::Result<()> {
        println!("{} {}", style("You:").cyan().bold(), content);
        Ok(())
    }

    pub fn print_ai_message(&mut self, content: &str) -> io::Result<()> {
        println!("{} {}", style("ARULA:").green().bold(), content);
        Ok(())
    }

    pub fn print_error(&mut self, content: &str) -> io::Result<()> {
        println!("{} {}", style("Error:").red().bold(), content);
        Ok(())
    }

    pub fn print_system(&mut self, content: &str) -> io::Result<()> {
        println!("{}", style(content).yellow().dim());
        Ok(())
    }

    pub fn print_tool_call(&mut self, name: &str, args: &str) -> io::Result<()> {
        if self.debug {
            println!("{} {}", style("🔧 Tool Call:").magenta().bold(), style(name).magenta());
            if !args.is_empty() {
                println!("   {}", style(format!("Args: {}", args)).dim());
            }
        }
        Ok(())
    }

    pub fn print_tool_result(&mut self, result: &str) -> io::Result<()> {
        let max_lines = if self.debug { 50 } else { 10 };
        let truncated_result = self.truncate_output(result, max_lines);
        if self.debug {
            println!("   {}", style(format!("Result: {}", truncated_result)).blue());
        } else {
            println!("   {}", style(truncated_result).blue());
        }
        Ok(())
    }

    fn truncate_output(&self, output: &str, max_lines: usize) -> String {
        let lines: Vec<&str> = output.lines().collect();

        if lines.len() <= max_lines {
            output.to_string()
        } else {
            let truncated_lines: Vec<String> = lines
                .iter()
                .take(max_lines)
                .map(|line| line.to_string())
                .collect();

            format!("{}\n... ({} more lines)", truncated_lines.join("\n"), lines.len() - max_lines)
        }
    }

    pub fn print_streaming_chunk(&mut self, chunk: &str) -> io::Result<()> {
        print!("{}", chunk);
        std::io::stdout().flush()?;
        Ok(())
    }

    pub fn start_ai_message(&mut self) -> io::Result<()> {
        print!("{} ", style("ARULA:").green().bold());
        std::io::stdout().flush()?;
        Ok(())
    }

    pub fn end_line(&mut self) -> io::Result<()> {
        println!();
        Ok(())
    }

    pub fn print_banner(&mut self) -> io::Result<()> {
        println!("{}", style("╔═══════════════════════════════════════╗").cyan().bold());
        println!("{}", style("║      ARULA - Autonomous AI CLI        ║").cyan().bold());
        println!("{}", style("╚═══════════════════════════════════════╝").cyan().bold());
        Ok(())
    }
}

impl Default for OutputHandler {
    fn default() -> Self {
        Self::new()
    }
}
