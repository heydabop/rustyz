pub type CommandError = Box<dyn std::error::Error + Send + Sync>;
pub type CommandResult = Result<(), CommandError>;
