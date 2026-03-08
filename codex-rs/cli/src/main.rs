use anyhow::Context;
use clap::Args;
use clap::CommandFactory;
use clap::Parser;
use clap_complete::Shell;
use clap_complete::generate;
use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;
use codex_chatgpt::apply_command::ApplyCommand;
use codex_chatgpt::apply_command::run_apply_command;
use codex_cli::LandlockCommand;
use codex_cli::SeatbeltCommand;
use codex_cli::WindowsCommand;
use codex_cli::login::read_api_key_from_stdin;
use codex_cli::login::run_login_status;
use codex_cli::login::run_login_with_api_key;
use codex_cli::login::run_login_with_chatgpt;
use codex_cli::login::run_login_with_device_code;
use codex_cli::login::run_logout;
use codex_cloud_tasks::Cli as CloudTasksCli;
use codex_exec::Cli as ExecCli;
use codex_exec::Command as ExecCommand;
use codex_exec::ReviewArgs;
use codex_execpolicy::ExecPolicyCheckCommand;
use codex_responses_api_proxy::Args as ResponsesApiProxyArgs;
use codex_state::StateRuntime;
use codex_state::state_db_path;
use codex_tui::AppExitInfo;
use codex_tui::Cli as TuiCli;
use codex_tui::ExitReason;
use codex_tui::update_action::UpdateAction;
use codex_utils_cli::CliConfigOverrides;
use owo_colors::OwoColorize;
use std::io::ErrorKind;
use std::io::IsTerminal;
use std::path::PathBuf;
use supports_color::Stream;

#[cfg(target_os = "macos")]
mod app_cmd;
#[cfg(target_os = "macos")]
mod desktop_app;
mod mcp_cmd;
#[cfg(not(windows))]
mod wsl_paths;

use crate::mcp_cmd::McpCli;

use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config::find_codex_home;
use codex_core::features::Stage;
use codex_core::features::is_known_feature_key;
use codex_core::terminal::TerminalName;

/// Codex CLI
///
/// If no subcommand is specified, options will be forwarded to the interactive CLI.
#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    // If a sub‑command is given, ignore requirements of the default args.
    subcommand_negates_reqs = true,
    // The executable is sometimes invoked via a platform‑specific name like
    // `codex-x86_64-unknown-linux-musl`, but the help output should always use
    // the generic `codex` command name that users run.
    bin_name = "codex",
    override_usage = "codex [OPTIONS] [PROMPT]\n       codex [OPTIONS] <COMMAND> [ARGS]"
)]
struct MultitoolCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    pub feature_toggles: FeatureToggles,

    #[clap(flatten)]
    interactive: TuiCli,

    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Run Codex non-interactively.
    #[clap(visible_alias = "e")]
    Exec(ExecCli),

    /// Run a code review non-interactively.
    Review(ReviewArgs),

    /// Manage login.
    Login(LoginCommand),

    /// Remove stored authentication credentials.
    Logout(LogoutCommand),

    /// Manage external MCP servers for Codex.
    Mcp(McpCli),

    /// Start Codex as an MCP server (stdio).
    McpServer,

    /// [experimental] Run the app server or related tooling.
    AppServer(AppServerCommand),

    /// Launch the Codex desktop app (downloads the macOS installer if missing).
    #[cfg(target_os = "macos")]
    App(app_cmd::AppCommand),

    /// Generate shell completion scripts.
    Completion(CompletionCommand),

    /// Run commands within a Codex-provided sandbox.
    Sandbox(SandboxArgs),

    /// Debugging tools.
    Debug(DebugCommand),

    /// Execpolicy tooling.
    #[clap(hide = true)]
    Execpolicy(ExecpolicyCommand),

    /// Apply the latest diff produced by Codex agent as a `git apply` to your local working tree.
    #[clap(visible_alias = "a")]
    Apply(ApplyCommand),

    /// Resume a previous interactive session (picker by default; use --last to continue the most recent).
    Resume(ResumeCommand),

    /// Fork a previous interactive session (picker by default; use --last to fork the most recent).
    Fork(ForkCommand),

    /// [EXPERIMENTAL] Browse tasks from Codex Cloud and apply changes locally.
    #[clap(name = "cloud", alias = "cloud-tasks")]
    Cloud(CloudTasksCli),

    /// Internal: run the responses API proxy.
    #[clap(hide = true)]
    ResponsesApiProxy(ResponsesApiProxyArgs),

    /// Internal: relay stdio to a Unix domain socket.
    #[clap(hide = true, name = "stdio-to-uds")]
    StdioToUds(StdioToUdsCommand),

    /// [EXPERIMENTAL] Run process-mode orchestration commands.
    Process(ProcessCli),

    /// Inspect feature flags.
    Features(FeaturesCli),
}

#[derive(Debug, Parser)]
struct CompletionCommand {
    /// Shell to generate completions for
    #[clap(value_enum, default_value_t = Shell::Bash)]
    shell: Shell,
}

#[derive(Debug, Parser)]
struct DebugCommand {
    #[command(subcommand)]
    subcommand: DebugSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum DebugSubcommand {
    /// Tooling: helps debug the app server.
    AppServer(DebugAppServerCommand),

    /// Internal: reset local memory state for a fresh start.
    #[clap(hide = true)]
    ClearMemories,
}

#[derive(Debug, Parser)]
struct DebugAppServerCommand {
    #[command(subcommand)]
    subcommand: DebugAppServerSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum DebugAppServerSubcommand {
    // Send message to app server V2.
    SendMessageV2(DebugAppServerSendMessageV2Command),
}

#[derive(Debug, Parser)]
struct DebugAppServerSendMessageV2Command {
    #[arg(value_name = "USER_MESSAGE", required = true)]
    user_message: String,
}

#[derive(Debug, Parser)]
struct ResumeCommand {
    /// Conversation/session id (UUID) or thread name. UUIDs take precedence if it parses.
    /// If omitted, use --last to pick the most recent recorded session.
    #[arg(value_name = "SESSION_ID")]
    session_id: Option<String>,

    /// Continue the most recent session without showing the picker.
    #[arg(long = "last", default_value_t = false)]
    last: bool,

    /// Show all sessions (disables cwd filtering and shows CWD column).
    #[arg(long = "all", default_value_t = false)]
    all: bool,

    #[clap(flatten)]
    config_overrides: TuiCli,
}

#[derive(Debug, Parser)]
struct ForkCommand {
    /// Conversation/session id (UUID). When provided, forks this session.
    /// If omitted, use --last to pick the most recent recorded session.
    #[arg(value_name = "SESSION_ID")]
    session_id: Option<String>,

    /// Fork the most recent session without showing the picker.
    #[arg(long = "last", default_value_t = false, conflicts_with = "session_id")]
    last: bool,

    /// Show all sessions (disables cwd filtering and shows CWD column).
    #[arg(long = "all", default_value_t = false)]
    all: bool,

    #[clap(flatten)]
    config_overrides: TuiCli,
}

#[derive(Debug, Parser)]
struct SandboxArgs {
    #[command(subcommand)]
    cmd: SandboxCommand,
}

#[derive(Debug, clap::Subcommand)]
enum SandboxCommand {
    /// Run a command under Seatbelt (macOS only).
    #[clap(visible_alias = "seatbelt")]
    Macos(SeatbeltCommand),

    /// Run a command under Landlock+seccomp (Linux only).
    #[clap(visible_alias = "landlock")]
    Linux(LandlockCommand),

    /// Run a command under Windows restricted token (Windows only).
    Windows(WindowsCommand),
}

#[derive(Debug, Parser)]
struct ExecpolicyCommand {
    #[command(subcommand)]
    sub: ExecpolicySubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum ExecpolicySubcommand {
    /// Check execpolicy files against a command.
    #[clap(name = "check")]
    Check(ExecPolicyCheckCommand),
}

#[derive(Debug, Parser)]
struct LoginCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,

    #[arg(
        long = "with-api-key",
        help = "Read the API key from stdin (e.g. `printenv OPENAI_API_KEY | codex login --with-api-key`)"
    )]
    with_api_key: bool,

    #[arg(
        long = "api-key",
        value_name = "API_KEY",
        help = "(deprecated) Previously accepted the API key directly; now exits with guidance to use --with-api-key",
        hide = true
    )]
    api_key: Option<String>,

    #[arg(long = "device-auth")]
    use_device_code: bool,

    /// EXPERIMENTAL: Use custom OAuth issuer base URL (advanced)
    /// Override the OAuth issuer base URL (advanced)
    #[arg(long = "experimental_issuer", value_name = "URL", hide = true)]
    issuer_base_url: Option<String>,

    /// EXPERIMENTAL: Use custom OAuth client ID (advanced)
    #[arg(long = "experimental_client-id", value_name = "CLIENT_ID", hide = true)]
    client_id: Option<String>,

    #[command(subcommand)]
    action: Option<LoginSubcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum LoginSubcommand {
    /// Show login status.
    Status,
}

#[derive(Debug, Parser)]
struct LogoutCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,
}

#[derive(Debug, Parser)]
struct AppServerCommand {
    /// Omit to run the app server; specify a subcommand for tooling.
    #[command(subcommand)]
    subcommand: Option<AppServerSubcommand>,

    /// Transport endpoint URL. Supported values: `stdio://` (default),
    /// `ws://IP:PORT`.
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = codex_app_server::AppServerTransport::DEFAULT_LISTEN_URL
    )]
    listen: codex_app_server::AppServerTransport,

    /// Controls whether analytics are enabled by default.
    ///
    /// Analytics are disabled by default for app-server. Users have to explicitly opt in
    /// via the `analytics` section in the config.toml file.
    ///
    /// However, for first-party use cases like the VSCode IDE extension, we default analytics
    /// to be enabled by default by setting this flag. Users can still opt out by setting this
    /// in their config.toml:
    ///
    /// ```toml
    /// [analytics]
    /// enabled = false
    /// ```
    ///
    /// See https://developers.openai.com/codex/config-advanced/#metrics for more details.
    #[arg(long = "analytics-default-enabled")]
    analytics_default_enabled: bool,
}

#[derive(Debug, clap::Subcommand)]
enum AppServerSubcommand {
    /// [experimental] Generate TypeScript bindings for the app server protocol.
    GenerateTs(GenerateTsCommand),

    /// [experimental] Generate JSON Schema for the app server protocol.
    GenerateJsonSchema(GenerateJsonSchemaCommand),
}

#[derive(Debug, Args)]
struct GenerateTsCommand {
    /// Output directory where .ts files will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Optional path to the Prettier executable to format generated files
    #[arg(short = 'p', long = "prettier", value_name = "PRETTIER_BIN")]
    prettier: Option<PathBuf>,

    /// Include experimental methods and fields in the generated output
    #[arg(long = "experimental", default_value_t = false)]
    experimental: bool,
}

#[derive(Debug, Args)]
struct GenerateJsonSchemaCommand {
    /// Output directory where the schema bundle will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Include experimental methods and fields in the generated output
    #[arg(long = "experimental", default_value_t = false)]
    experimental: bool,
}

#[derive(Debug, Parser)]
struct StdioToUdsCommand {
    /// Path to the Unix domain socket to connect to.
    #[arg(value_name = "SOCKET_PATH")]
    socket_path: PathBuf,
}

fn format_exit_messages(exit_info: AppExitInfo, color_enabled: bool) -> Vec<String> {
    let AppExitInfo {
        token_usage,
        thread_id: conversation_id,
        thread_name,
        ..
    } = exit_info;

    if token_usage.is_zero() {
        return Vec::new();
    }

    let mut lines = vec![format!(
        "{}",
        codex_protocol::protocol::FinalOutput::from(token_usage)
    )];

    if let Some(resume_cmd) =
        codex_core::util::resume_command(thread_name.as_deref(), conversation_id)
    {
        let command = if color_enabled {
            resume_cmd.cyan().to_string()
        } else {
            resume_cmd
        };
        lines.push(format!("To continue this session, run {command}"));
    }

    lines
}

/// Handle the app exit and print the results. Optionally run the update action.
fn handle_app_exit(exit_info: AppExitInfo) -> anyhow::Result<()> {
    match exit_info.exit_reason {
        ExitReason::Fatal(message) => {
            eprintln!("ERROR: {message}");
            std::process::exit(1);
        }
        ExitReason::UserRequested => { /* normal exit */ }
    }

    let update_action = exit_info.update_action;
    let color_enabled = supports_color::on(Stream::Stdout).is_some();
    for line in format_exit_messages(exit_info, color_enabled) {
        println!("{line}");
    }
    if let Some(action) = update_action {
        run_update_action(action)?;
    }
    Ok(())
}

/// Run the update action and print the result.
fn run_update_action(action: UpdateAction) -> anyhow::Result<()> {
    println!();
    let cmd_str = action.command_str();
    println!("Updating Codex via `{cmd_str}`...");

    let status = {
        #[cfg(windows)]
        {
            // On Windows, run via cmd.exe so .CMD/.BAT are correctly resolved (PATHEXT semantics).
            std::process::Command::new("cmd")
                .args(["/C", &cmd_str])
                .status()?
        }
        #[cfg(not(windows))]
        {
            let (cmd, args) = action.command_args();
            let command_path = crate::wsl_paths::normalize_for_wsl(cmd);
            let normalized_args: Vec<String> = args
                .iter()
                .map(crate::wsl_paths::normalize_for_wsl)
                .collect();
            std::process::Command::new(&command_path)
                .args(&normalized_args)
                .status()?
        }
    };
    if !status.success() {
        anyhow::bail!("`{cmd_str}` failed with status {status}");
    }
    println!("\n🎉 Update ran successfully! Please restart Codex.");
    Ok(())
}

fn run_execpolicycheck(cmd: ExecPolicyCheckCommand) -> anyhow::Result<()> {
    cmd.run()
}

async fn run_debug_app_server_command(cmd: DebugAppServerCommand) -> anyhow::Result<()> {
    match cmd.subcommand {
        DebugAppServerSubcommand::SendMessageV2(cmd) => {
            let codex_bin = std::env::current_exe()?;
            codex_app_server_test_client::send_message_v2(&codex_bin, &[], cmd.user_message, &None)
                .await
        }
    }
}

#[derive(Debug, Default, Parser, Clone)]
struct FeatureToggles {
    /// Enable a feature (repeatable). Equivalent to `-c features.<name>=true`.
    #[arg(long = "enable", value_name = "FEATURE", action = clap::ArgAction::Append, global = true)]
    enable: Vec<String>,

    /// Disable a feature (repeatable). Equivalent to `-c features.<name>=false`.
    #[arg(long = "disable", value_name = "FEATURE", action = clap::ArgAction::Append, global = true)]
    disable: Vec<String>,
}

impl FeatureToggles {
    fn to_overrides(&self) -> anyhow::Result<Vec<String>> {
        let mut v = Vec::new();
        for feature in &self.enable {
            Self::validate_feature(feature)?;
            v.push(format!("features.{feature}=true"));
        }
        for feature in &self.disable {
            Self::validate_feature(feature)?;
            v.push(format!("features.{feature}=false"));
        }
        Ok(v)
    }

    fn validate_feature(feature: &str) -> anyhow::Result<()> {
        if is_known_feature_key(feature) {
            Ok(())
        } else {
            anyhow::bail!("Unknown feature flag: {feature}")
        }
    }
}

#[derive(Debug, Parser)]
struct FeaturesCli {
    #[command(subcommand)]
    sub: FeaturesSubcommand,
}

#[derive(Debug, Parser)]
enum FeaturesSubcommand {
    /// List known features with their stage and effective state.
    List,
    /// Enable a feature in config.toml.
    Enable(FeatureSetArgs),
    /// Disable a feature in config.toml.
    Disable(FeatureSetArgs),
}

#[derive(Debug, Parser)]
struct FeatureSetArgs {
    /// Feature key to update (for example: unified_exec).
    feature: String,
}

#[derive(Debug, Parser)]
struct ProcessCli {
    #[command(subcommand)]
    sub: ProcessSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum ProcessSubcommand {
    /// Start a process-mode run and scaffold required artifacts.
    Run(ProcessRunArgs),
    /// Inspect a process-mode run by id, or the latest run if omitted.
    Status(ProcessStatusArgs),
    /// Ingest open PR comments from GitHub and scaffold response planning artifacts.
    #[clap(name = "pr-comments")]
    PrComments(ProcessPrCommentsArgs),
    /// Watch open issues for process automation.
    Issues(ProcessIssuesCli),
}

#[derive(Debug, Args)]
struct ProcessRunArgs {
    /// Task description for this run.
    #[arg(long)]
    task: String,
}

#[derive(Debug, Args)]
struct ProcessStatusArgs {
    /// Run id to inspect (defaults to latest run in .process/runs).
    #[arg(long)]
    run_id: Option<String>,
}

#[derive(Debug, Args)]
struct ProcessPrCommentsArgs {
    /// Repository in owner/name format.
    #[arg(long)]
    repo: String,

    /// Pull request number.
    #[arg(long)]
    pr: u64,

    /// Triage comments and create follow-up issues for deferred work.
    #[arg(long, default_value_t = false)]
    act: bool,
}

#[derive(Debug, Args)]
struct ProcessIssuesCli {
    #[command(subcommand)]
    sub: ProcessIssuesSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum ProcessIssuesSubcommand {
    /// Fetch matching open issues and produce an automation action plan artifact.
    Watch(ProcessIssuesWatchArgs),
}

#[derive(Debug, Args)]
struct ProcessIssuesWatchArgs {
    /// Repository in owner/name format.
    #[arg(long)]
    repo: String,

    /// Label to filter issues.
    #[arg(long)]
    label: String,

    /// Max number of issues to fetch.
    #[arg(long, default_value_t = 20)]
    limit: u32,

    /// Triage matching issues and attempt targeted quick-fix automation.
    #[arg(long, default_value_t = false)]
    act: bool,
}

fn stage_str(stage: codex_core::features::Stage) -> &'static str {
    use codex_core::features::Stage;
    match stage {
        Stage::UnderDevelopment => "under development",
        Stage::Experimental { .. } => "experimental",
        Stage::Stable => "stable",
        Stage::Deprecated => "deprecated",
        Stage::Removed => "removed",
    }
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        cli_main(arg0_paths).await?;
        Ok(())
    })
}

async fn cli_main(arg0_paths: Arg0DispatchPaths) -> anyhow::Result<()> {
    let MultitoolCli {
        config_overrides: mut root_config_overrides,
        feature_toggles,
        mut interactive,
        subcommand,
    } = MultitoolCli::parse();

    // Fold --enable/--disable into config overrides so they flow to all subcommands.
    let toggle_overrides = feature_toggles.to_overrides()?;
    root_config_overrides.raw_overrides.extend(toggle_overrides);

    match subcommand {
        None => {
            prepend_config_flags(
                &mut interactive.config_overrides,
                root_config_overrides.clone(),
            );
            let exit_info = run_interactive_tui(interactive, arg0_paths.clone()).await?;
            handle_app_exit(exit_info)?;
        }
        Some(Subcommand::Exec(mut exec_cli)) => {
            prepend_config_flags(
                &mut exec_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_exec::run_main(exec_cli, arg0_paths.clone()).await?;
        }
        Some(Subcommand::Review(review_args)) => {
            let mut exec_cli = ExecCli::try_parse_from(["codex", "exec"])?;
            exec_cli.command = Some(ExecCommand::Review(review_args));
            prepend_config_flags(
                &mut exec_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_exec::run_main(exec_cli, arg0_paths.clone()).await?;
        }
        Some(Subcommand::McpServer) => {
            codex_mcp_server::run_main(arg0_paths.clone(), root_config_overrides).await?;
        }
        Some(Subcommand::Mcp(mut mcp_cli)) => {
            // Propagate any root-level config overrides (e.g. `-c key=value`).
            prepend_config_flags(&mut mcp_cli.config_overrides, root_config_overrides.clone());
            mcp_cli.run().await?;
        }
        Some(Subcommand::AppServer(app_server_cli)) => match app_server_cli.subcommand {
            None => {
                let transport = app_server_cli.listen;
                codex_app_server::run_main_with_transport(
                    arg0_paths.clone(),
                    root_config_overrides,
                    codex_core::config_loader::LoaderOverrides::default(),
                    app_server_cli.analytics_default_enabled,
                    transport,
                )
                .await?;
            }
            Some(AppServerSubcommand::GenerateTs(gen_cli)) => {
                let options = codex_app_server_protocol::GenerateTsOptions {
                    experimental_api: gen_cli.experimental,
                    ..Default::default()
                };
                codex_app_server_protocol::generate_ts_with_options(
                    &gen_cli.out_dir,
                    gen_cli.prettier.as_deref(),
                    options,
                )?;
            }
            Some(AppServerSubcommand::GenerateJsonSchema(gen_cli)) => {
                codex_app_server_protocol::generate_json_with_experimental(
                    &gen_cli.out_dir,
                    gen_cli.experimental,
                )?;
            }
        },
        #[cfg(target_os = "macos")]
        Some(Subcommand::App(app_cli)) => {
            app_cmd::run_app(app_cli).await?;
        }
        Some(Subcommand::Resume(ResumeCommand {
            session_id,
            last,
            all,
            config_overrides,
        })) => {
            interactive = finalize_resume_interactive(
                interactive,
                root_config_overrides.clone(),
                session_id,
                last,
                all,
                config_overrides,
            );
            let exit_info = run_interactive_tui(interactive, arg0_paths.clone()).await?;
            handle_app_exit(exit_info)?;
        }
        Some(Subcommand::Fork(ForkCommand {
            session_id,
            last,
            all,
            config_overrides,
        })) => {
            interactive = finalize_fork_interactive(
                interactive,
                root_config_overrides.clone(),
                session_id,
                last,
                all,
                config_overrides,
            );
            let exit_info = run_interactive_tui(interactive, arg0_paths.clone()).await?;
            handle_app_exit(exit_info)?;
        }
        Some(Subcommand::Login(mut login_cli)) => {
            prepend_config_flags(
                &mut login_cli.config_overrides,
                root_config_overrides.clone(),
            );
            match login_cli.action {
                Some(LoginSubcommand::Status) => {
                    run_login_status(login_cli.config_overrides).await;
                }
                None => {
                    if login_cli.use_device_code {
                        run_login_with_device_code(
                            login_cli.config_overrides,
                            login_cli.issuer_base_url,
                            login_cli.client_id,
                        )
                        .await;
                    } else if login_cli.api_key.is_some() {
                        eprintln!(
                            "The --api-key flag is no longer supported. Pipe the key instead, e.g. `printenv OPENAI_API_KEY | codex login --with-api-key`."
                        );
                        std::process::exit(1);
                    } else if login_cli.with_api_key {
                        let api_key = read_api_key_from_stdin();
                        run_login_with_api_key(login_cli.config_overrides, api_key).await;
                    } else {
                        run_login_with_chatgpt(login_cli.config_overrides).await;
                    }
                }
            }
        }
        Some(Subcommand::Logout(mut logout_cli)) => {
            prepend_config_flags(
                &mut logout_cli.config_overrides,
                root_config_overrides.clone(),
            );
            run_logout(logout_cli.config_overrides).await;
        }
        Some(Subcommand::Completion(completion_cli)) => {
            print_completion(completion_cli);
        }
        Some(Subcommand::Cloud(mut cloud_cli)) => {
            prepend_config_flags(
                &mut cloud_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_cloud_tasks::run_main(cloud_cli, arg0_paths.codex_linux_sandbox_exe.clone())
                .await?;
        }
        Some(Subcommand::Sandbox(sandbox_args)) => match sandbox_args.cmd {
            SandboxCommand::Macos(mut seatbelt_cli) => {
                prepend_config_flags(
                    &mut seatbelt_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::debug_sandbox::run_command_under_seatbelt(
                    seatbelt_cli,
                    arg0_paths.codex_linux_sandbox_exe.clone(),
                )
                .await?;
            }
            SandboxCommand::Linux(mut landlock_cli) => {
                prepend_config_flags(
                    &mut landlock_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::debug_sandbox::run_command_under_landlock(
                    landlock_cli,
                    arg0_paths.codex_linux_sandbox_exe.clone(),
                )
                .await?;
            }
            SandboxCommand::Windows(mut windows_cli) => {
                prepend_config_flags(
                    &mut windows_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::debug_sandbox::run_command_under_windows(
                    windows_cli,
                    arg0_paths.codex_linux_sandbox_exe.clone(),
                )
                .await?;
            }
        },
        Some(Subcommand::Debug(DebugCommand { subcommand })) => match subcommand {
            DebugSubcommand::AppServer(cmd) => {
                run_debug_app_server_command(cmd).await?;
            }
            DebugSubcommand::ClearMemories => {
                run_debug_clear_memories_command(&root_config_overrides, &interactive).await?;
            }
        },
        Some(Subcommand::Execpolicy(ExecpolicyCommand { sub })) => match sub {
            ExecpolicySubcommand::Check(cmd) => run_execpolicycheck(cmd)?,
        },
        Some(Subcommand::Apply(mut apply_cli)) => {
            prepend_config_flags(
                &mut apply_cli.config_overrides,
                root_config_overrides.clone(),
            );
            run_apply_command(apply_cli, None).await?;
        }
        Some(Subcommand::ResponsesApiProxy(args)) => {
            tokio::task::spawn_blocking(move || codex_responses_api_proxy::run_main(args))
                .await??;
        }
        Some(Subcommand::StdioToUds(cmd)) => {
            let socket_path = cmd.socket_path;
            tokio::task::spawn_blocking(move || codex_stdio_to_uds::run(socket_path.as_path()))
                .await??;
        }
        Some(Subcommand::Process(ProcessCli { sub })) => match sub {
            ProcessSubcommand::Run(args) => {
                run_process_mode(args)?;
            }
            ProcessSubcommand::Status(args) => {
                process_mode_status(args)?;
            }
            ProcessSubcommand::PrComments(args) => {
                process_mode_pr_comments(args)?;
            }
            ProcessSubcommand::Issues(ProcessIssuesCli { sub }) => match sub {
                ProcessIssuesSubcommand::Watch(args) => {
                    process_mode_issues_watch(args)?;
                }
            },
        },
        Some(Subcommand::Features(FeaturesCli { sub })) => match sub {
            FeaturesSubcommand::List => {
                // Respect root-level `-c` overrides plus top-level flags like `--profile`.
                let mut cli_kv_overrides = root_config_overrides
                    .parse_overrides()
                    .map_err(anyhow::Error::msg)?;

                // Honor `--search` via the canonical web_search mode.
                if interactive.web_search {
                    cli_kv_overrides.push((
                        "web_search".to_string(),
                        toml::Value::String("live".to_string()),
                    ));
                }

                // Thread through relevant top-level flags (at minimum, `--profile`).
                let overrides = ConfigOverrides {
                    config_profile: interactive.config_profile.clone(),
                    ..Default::default()
                };

                let config = Config::load_with_cli_overrides_and_harness_overrides(
                    cli_kv_overrides,
                    overrides,
                )
                .await?;
                let mut rows = Vec::with_capacity(codex_core::features::FEATURES.len());
                let mut name_width = 0;
                let mut stage_width = 0;
                for def in codex_core::features::FEATURES.iter() {
                    let name = def.key;
                    let stage = stage_str(def.stage);
                    let enabled = config.features.enabled(def.id);
                    name_width = name_width.max(name.len());
                    stage_width = stage_width.max(stage.len());
                    rows.push((name, stage, enabled));
                }
                rows.sort_unstable_by_key(|(name, _, _)| *name);

                for (name, stage, enabled) in rows {
                    println!("{name:<name_width$}  {stage:<stage_width$}  {enabled}");
                }
            }
            FeaturesSubcommand::Enable(FeatureSetArgs { feature }) => {
                enable_feature_in_config(&interactive, &feature).await?;
            }
            FeaturesSubcommand::Disable(FeatureSetArgs { feature }) => {
                disable_feature_in_config(&interactive, &feature).await?;
            }
        },
    }

    Ok(())
}

fn run_process_mode(args: ProcessRunArgs) -> anyhow::Result<()> {
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let run_dir = std::path::PathBuf::from(".process/runs").join(&run_id);
    std::fs::create_dir_all(&run_dir)?;

    let contract = serde_json::json!({
        "run_id": run_id,
        "state": "CONTRACT",
        "task": args.task,
        "status": "pending"
    });
    std::fs::write(
        run_dir.join("contract.json"),
        serde_json::to_string_pretty(&contract)?,
    )?;

    let red_proof = serde_json::json!({
        "state": "RED",
        "status": "pending",
        "required": "capture failing test proof"
    });
    std::fs::write(
        run_dir.join("red-proof.json"),
        serde_json::to_string_pretty(&red_proof)?,
    )?;

    let verify = serde_json::json!({
        "state": "VERIFY",
        "status": "pending",
        "required": ["lint", "tests"]
    });
    std::fs::write(
        run_dir.join("verify.json"),
        serde_json::to_string_pretty(&verify)?,
    )?;

    let traceability = serde_json::json!({
        "state": "EVIDENCE",
        "status": "pending",
        "ac_to_tests_to_files": []
    });
    std::fs::write(
        run_dir.join("traceability.json"),
        serde_json::to_string_pretty(&traceability)?,
    )?;

    let summary = format!(
        "# Process Run {run_id}\n\n- task: {}\n- state: INTAKE\n- status: bootstrapped\n",
        contract["task"].as_str().unwrap_or_default()
    );
    std::fs::write(run_dir.join("summary.md"), summary)?;

    println!("Process run bootstrapped: {run_id}");
    println!("Artifacts: {}", run_dir.display());
    Ok(())
}

fn process_mode_status(args: ProcessStatusArgs) -> anyhow::Result<()> {
    let runs_dir = std::path::PathBuf::from(".process/runs");
    let run_id = if let Some(run_id) = args.run_id {
        run_id
    } else {
        let mut run_ids = std::fs::read_dir(&runs_dir)?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        run_ids.sort();
        run_ids
            .pop()
            .ok_or_else(|| anyhow::anyhow!("No process runs found in {}", runs_dir.display()))?
    };

    let run_dir = runs_dir.join(&run_id);
    if !run_dir.exists() {
        return Err(anyhow::anyhow!("Run not found: {}", run_dir.display()));
    }

    println!("Run: {run_id}");
    println!("Directory: {}", run_dir.display());
    for name in [
        "contract.json",
        "red-proof.json",
        "verify.json",
        "traceability.json",
        "summary.md",
    ] {
        let path = run_dir.join(name);
        let status = if path.exists() { "present" } else { "missing" };
        println!("- {name}: {status}");
    }
    Ok(())
}

fn process_mode_pr_comments(args: ProcessPrCommentsArgs) -> anyhow::Result<()> {
    let (owner, name) = parse_repo_owner_and_name(&args.repo)?;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let run_dir = std::path::PathBuf::from(".process/runs").join(&run_id);
    std::fs::create_dir_all(&run_dir)?;

    let unresolved_review_comments = fetch_unresolved_review_comments(&owner, &name, args.pr)
        .with_context(|| {
            format!(
                "Failed to fetch unresolved review comments for {}#{}",
                args.repo, args.pr
            )
        })?;
    let open_issue_comments = fetch_issue_comments(&args.repo, args.pr).with_context(|| {
        format!(
            "Failed to fetch issue comments for {}#{}",
            args.repo, args.pr
        )
    })?;
    let grouped_by_type = GroupedByType {
        unresolved_review_comments: unresolved_review_comments.len(),
        open_issue_comments: open_issue_comments.len(),
        total: unresolved_review_comments.len() + open_issue_comments.len(),
    };
    let suggested_next_actions = suggested_next_actions(&grouped_by_type);
    let payload = ProcessPrCommentsArtifact {
        repo: args.repo.clone(),
        pr: args.pr,
        fetched_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        unresolved_review_comments,
        open_issue_comments,
        grouped_by_type,
        suggested_next_actions,
    };
    std::fs::write(
        run_dir.join("pr-comments.json"),
        serde_json::to_string_pretty(&payload)?,
    )?;

    if !args.act {
        println!("PR comment response run created: {run_id}");
        println!("Target: {}#{}", args.repo, args.pr);
        println!("Artifact: {}", run_dir.join("pr-comments.json").display());
        return Ok(());
    }

    let mut triage_items = payload
        .unresolved_review_comments
        .iter()
        .map(|comment| ProcessCommentTriageItem {
            source: "review_comment".to_string(),
            comment_id: comment.id.clone(),
            review_thread_id: comment.review_thread_id.clone(),
            author: comment.author.clone(),
            body: comment.body.clone(),
            comment_url: comment.url.clone(),
            decision: classify_comment_triage(&comment.body),
            created_issue_url: None,
            todo: None,
            quick_fix_attempted: false,
            quick_fix_success: None,
            quick_fix_summary: None,
            quick_fix_error: None,
            quick_fix_branch: None,
            quick_fix_commit_sha: None,
            quick_fix_commit_url: None,
            quick_fix_pushed: None,
            quick_fix_remote_branch: None,
            quick_fix_pr_url: None,
            quick_fix_pr_number: None,
            quick_fix_push_error: None,
            quick_fix_pr_error: None,
            quick_fix_thread_resolved: None,
            quick_fix_thread_resolve_error: None,
        })
        .collect::<Vec<_>>();
    triage_items.extend(payload.open_issue_comments.iter().map(|comment| {
        ProcessCommentTriageItem {
            source: "issue_comment".to_string(),
            comment_id: comment.id.clone(),
            review_thread_id: None,
            author: comment.author.clone(),
            body: comment.body.clone(),
            comment_url: comment.url.clone(),
            decision: classify_comment_triage(&comment.body),
            created_issue_url: None,
            todo: None,
            quick_fix_attempted: false,
            quick_fix_success: None,
            quick_fix_summary: None,
            quick_fix_error: None,
            quick_fix_branch: None,
            quick_fix_commit_sha: None,
            quick_fix_commit_url: None,
            quick_fix_pushed: None,
            quick_fix_remote_branch: None,
            quick_fix_pr_url: None,
            quick_fix_pr_number: None,
            quick_fix_push_error: None,
            quick_fix_pr_error: None,
            quick_fix_thread_resolved: None,
            quick_fix_thread_resolve_error: None,
        }
    }));

    let mut grouped_by_decision = ProcessTriageCounts::default();
    let mut created_issues = Vec::new();
    let mut successful_quick_fixes = Vec::new();
    let quick_fix_worktree_root = run_dir.join("quick-fix-worktrees");
    let quick_fix_root_error = std::fs::create_dir_all(&quick_fix_worktree_root)
        .err()
        .map(|err| {
            format!(
                "Unable to prepare quick-fix worktree directory {}: {err}",
                quick_fix_worktree_root.display()
            )
        });
    let quick_fix_base_sha = current_git_head_sha();
    let quick_fix_pr_base_branch = match fetch_pr_base_branch_name(&args.repo, args.pr) {
        Ok(branch) => branch,
        Err(err) => {
            eprintln!(
                "Warning: unable to determine source PR base branch for #{pr}: {err}; defaulting to `main` for follow-up PRs.",
                pr = args.pr
            );
            None
        }
    };
    for item in &mut triage_items {
        match item.decision {
            TriageDecision::QuickFix => {
                grouped_by_decision.quick_fix += 1;
                let execution = if let Some(err) = &quick_fix_root_error {
                    QuickFixExecutionResult {
                        attempted: false,
                        success: false,
                        summary: None,
                        error: Some(err.clone()),
                        files: Vec::new(),
                        verification: None,
                        branch_name: None,
                        commit_sha: None,
                        commit_url: None,
                        pushed: None,
                        remote_branch: None,
                        follow_up_pr_url: None,
                        follow_up_pr_number: None,
                        push_error: None,
                        pr_error: None,
                    }
                } else if let Ok(base_sha) = &quick_fix_base_sha {
                    run_quick_fix_item_in_isolated_branch(
                        &args.repo,
                        args.pr,
                        item,
                        base_sha,
                        &quick_fix_worktree_root,
                        quick_fix_pr_base_branch.as_deref().unwrap_or("main"),
                    )
                } else {
                    QuickFixExecutionResult {
                        attempted: false,
                        success: false,
                        summary: None,
                        error: Some(format!(
                            "Unable to read current git HEAD for quick-fix branching: {}",
                            quick_fix_base_sha
                                .as_ref()
                                .err()
                                .map_or_else(|| "unknown error".to_string(), ToString::to_string)
                        )),
                        files: Vec::new(),
                        verification: None,
                        branch_name: None,
                        commit_sha: None,
                        commit_url: None,
                        pushed: None,
                        remote_branch: None,
                        follow_up_pr_url: None,
                        follow_up_pr_number: None,
                        push_error: None,
                        pr_error: None,
                    }
                };
                let execution_summary = if execution.success {
                    Some(
                        execution
                            .summary
                            .clone()
                            .unwrap_or_else(|| "Applied targeted quick fix.".to_string()),
                    )
                } else {
                    execution.summary.clone()
                };
                item.quick_fix_attempted = execution.attempted;
                item.quick_fix_success = Some(execution.success);
                item.quick_fix_summary = execution_summary.clone();
                item.quick_fix_error = execution.error.clone();
                item.quick_fix_branch = execution.branch_name.clone();
                item.quick_fix_commit_sha = execution.commit_sha.clone();
                item.quick_fix_commit_url = execution.commit_url.clone();
                item.quick_fix_pushed = execution.pushed;
                item.quick_fix_remote_branch = execution.remote_branch.clone();
                item.quick_fix_pr_url = execution.follow_up_pr_url.clone();
                item.quick_fix_pr_number = execution.follow_up_pr_number;
                item.quick_fix_push_error = execution.push_error.clone();
                item.quick_fix_pr_error = execution.pr_error.clone();
                if execution.follow_up_pr_url.is_some()
                    && item.source == "review_comment"
                    && let Some(review_thread_id) = item.review_thread_id.as_deref()
                {
                    match resolve_review_thread(review_thread_id) {
                        Ok(()) => {
                            item.quick_fix_thread_resolved = Some(true);
                            item.quick_fix_thread_resolve_error = None;
                        }
                        Err(err) => {
                            item.quick_fix_thread_resolved = Some(false);
                            item.quick_fix_thread_resolve_error = Some(err.to_string());
                        }
                    }
                }
                if execution.success {
                    let summary = execution_summary
                        .unwrap_or_else(|| "Applied targeted quick fix.".to_string());
                    successful_quick_fixes.push(QuickFixSummary {
                        comment_id: item.comment_id.clone(),
                        summary,
                        files: execution.files,
                        verification: execution.verification,
                        commit_sha: execution.commit_sha,
                        commit_url: execution.commit_url,
                        follow_up_pr_url: execution.follow_up_pr_url,
                        follow_up_pr_number: execution.follow_up_pr_number,
                        thread_resolved: item.quick_fix_thread_resolved,
                        thread_resolve_error: item.quick_fix_thread_resolve_error.clone(),
                    });
                } else {
                    item.todo = Some(format!(
                        "TODO: manual quick fix for comment {comment_id} ({comment_url})",
                        comment_id = item.comment_id.as_str(),
                        comment_url = item.comment_url.as_str()
                    ));
                }
            }
            TriageDecision::NeedsIssue => {
                grouped_by_decision.needs_issue += 1;
                let issue_url = create_follow_up_issue(
                    &args.repo,
                    args.pr,
                    &item.author,
                    &item.comment_url,
                    &item.body,
                )?;
                item.created_issue_url = Some(issue_url.clone());
                created_issues.push(ProcessCreatedIssue {
                    source_comment_id: item.comment_id.clone(),
                    source_comment_url: item.comment_url.clone(),
                    issue_url,
                });
            }
            TriageDecision::Question => {
                grouped_by_decision.question += 1;
            }
        }
    }
    let pr_update_comment_url = if successful_quick_fixes.is_empty() {
        None
    } else {
        post_quick_fix_pr_update_comment(&args.repo, args.pr, &successful_quick_fixes)?
    };

    let triage_artifact = ProcessPrCommentsTriageArtifact {
        repo: args.repo.clone(),
        pr: args.pr,
        fetched_at: payload.fetched_at,
        triage_items,
        grouped_by_decision,
        created_issues: created_issues.clone(),
        pr_update_comment_url: pr_update_comment_url.clone(),
    };
    std::fs::write(
        run_dir.join("triage.json"),
        serde_json::to_string_pretty(&triage_artifact)?,
    )?;

    println!("PR comment action run created: {run_id}");
    println!("Target: {}#{}", args.repo, args.pr);
    println!("Ingested comments: {}", payload.grouped_by_type.total);
    println!(
        "Triage counts: quick_fix={}, needs_issue={}, question={}",
        triage_artifact.grouped_by_decision.quick_fix,
        triage_artifact.grouped_by_decision.needs_issue,
        triage_artifact.grouped_by_decision.question
    );
    println!("Artifact: {}", run_dir.join("triage.json").display());
    if created_issues.is_empty() {
        println!("Created issue URLs: none");
    } else {
        println!("Created issue URLs:");
        for issue in created_issues {
            println!("- {}", issue.issue_url);
        }
    }
    if let Some(url) = pr_update_comment_url {
        println!("PR update comment URL: {url}");
    } else {
        println!("PR update comment URL: none");
    }
    Ok(())
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessPrCommentsArtifact {
    repo: String,
    pr: u64,
    fetched_at: u64,
    unresolved_review_comments: Vec<UnresolvedReviewComment>,
    open_issue_comments: Vec<OpenIssueComment>,
    grouped_by_type: GroupedByType,
    suggested_next_actions: Vec<String>,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
struct UnresolvedReviewComment {
    id: String,
    review_thread_id: Option<String>,
    author: String,
    path: String,
    line: Option<u64>,
    body: String,
    url: String,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
struct OpenIssueComment {
    id: String,
    author: String,
    body: String,
    url: String,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct GroupedByType {
    unresolved_review_comments: usize,
    open_issue_comments: usize,
    total: usize,
}

#[derive(Debug, serde::Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum TriageDecision {
    QuickFix,
    NeedsIssue,
    Question,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessCommentTriageItem {
    source: String,
    comment_id: String,
    review_thread_id: Option<String>,
    author: String,
    body: String,
    comment_url: String,
    decision: TriageDecision,
    created_issue_url: Option<String>,
    todo: Option<String>,
    quick_fix_attempted: bool,
    quick_fix_success: Option<bool>,
    quick_fix_summary: Option<String>,
    quick_fix_error: Option<String>,
    quick_fix_branch: Option<String>,
    quick_fix_commit_sha: Option<String>,
    quick_fix_commit_url: Option<String>,
    quick_fix_pushed: Option<bool>,
    quick_fix_remote_branch: Option<String>,
    quick_fix_pr_url: Option<String>,
    quick_fix_pr_number: Option<u64>,
    quick_fix_push_error: Option<String>,
    quick_fix_pr_error: Option<String>,
    quick_fix_thread_resolved: Option<bool>,
    quick_fix_thread_resolve_error: Option<String>,
}

#[derive(Debug, serde::Serialize, Clone, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessTriageCounts {
    quick_fix: usize,
    needs_issue: usize,
    question: usize,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessCreatedIssue {
    source_comment_id: String,
    source_comment_url: String,
    issue_url: String,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessPrCommentsTriageArtifact {
    repo: String,
    pr: u64,
    fetched_at: u64,
    triage_items: Vec<ProcessCommentTriageItem>,
    grouped_by_decision: ProcessTriageCounts,
    created_issues: Vec<ProcessCreatedIssue>,
    pr_update_comment_url: Option<String>,
}

#[derive(Debug, Clone)]
struct QuickFixExecutionResult {
    attempted: bool,
    success: bool,
    summary: Option<String>,
    error: Option<String>,
    files: Vec<String>,
    verification: Option<String>,
    branch_name: Option<String>,
    commit_sha: Option<String>,
    commit_url: Option<String>,
    pushed: Option<bool>,
    remote_branch: Option<String>,
    follow_up_pr_url: Option<String>,
    follow_up_pr_number: Option<u64>,
    push_error: Option<String>,
    pr_error: Option<String>,
}

#[derive(Debug, Clone)]
struct QuickFixSummary {
    comment_id: String,
    summary: String,
    files: Vec<String>,
    verification: Option<String>,
    commit_sha: Option<String>,
    commit_url: Option<String>,
    follow_up_pr_url: Option<String>,
    follow_up_pr_number: Option<u64>,
    thread_resolved: Option<bool>,
    thread_resolve_error: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ParsedQuickFixOutput {
    summary: Option<String>,
    files: Vec<String>,
    verification: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ParsedGhPrCreateOutput {
    url: Option<String>,
    number: Option<u64>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ParsedGhIssueCommentOutput {
    url: Option<String>,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessIssuesWatchArtifact {
    fetched_at: u64,
    repo: String,
    label: String,
    open_issues: Vec<ProcessWatchIssue>,
    suggested_actions: Vec<String>,
}

#[derive(Debug, serde::Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum IssueWatchDecision {
    QuickFix,
    NeedsManual,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessIssuesWatchActIssueAction {
    issue_number: u64,
    issue_url: String,
    decision: IssueWatchDecision,
    attempted: bool,
    success: bool,
    branch: Option<String>,
    commit_sha: Option<String>,
    commit_url: Option<String>,
    pr_url: Option<String>,
    pr_number: Option<u64>,
    update_comment_url: Option<String>,
    error: Option<String>,
}

#[derive(Debug, serde::Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessIssuesWatchActArtifact {
    fetched_at: u64,
    repo: String,
    label: String,
    issue_actions: Vec<ProcessIssuesWatchActIssueAction>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProcessWatchIssue {
    number: u64,
    title: String,
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessWatchIssueCandidate {
    number: u64,
    title: String,
    body: String,
    url: String,
}

#[derive(Debug, serde::Deserialize)]
struct GhGraphQlResponse {
    data: Option<GhGraphQlData>,
    errors: Option<Vec<GhGraphQlError>>,
}

#[derive(Debug, serde::Deserialize)]
struct GhGraphQlError {
    message: String,
}

#[derive(Debug, serde::Deserialize)]
struct GhGraphQlData {
    repository: Option<GhRepository>,
}

#[derive(Debug, serde::Deserialize)]
struct GhResolveReviewThreadResponse {
    data: Option<GhResolveReviewThreadData>,
    errors: Option<Vec<GhGraphQlError>>,
}

#[derive(Debug, serde::Deserialize)]
struct GhResolveReviewThreadData {
    #[serde(rename = "resolveReviewThread")]
    resolve_review_thread: Option<GhResolveReviewThreadPayload>,
}

#[derive(Debug, serde::Deserialize)]
struct GhResolveReviewThreadPayload {
    thread: Option<GhResolveReviewThreadNode>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhResolveReviewThreadNode {
    id: String,
    is_resolved: bool,
}

#[derive(Debug, serde::Deserialize)]
struct GhRepository {
    #[serde(rename = "pullRequest")]
    pull_request: Option<GhPullRequest>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequest {
    review_threads: GhReviewThreadConnection,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewThreadConnection {
    nodes: Vec<GhReviewThreadNode>,
    page_info: GhPageInfo,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewThreadNode {
    id: String,
    is_resolved: bool,
    comments: GhReviewCommentConnection,
}

#[derive(Debug, serde::Deserialize)]
struct GhReviewCommentConnection {
    nodes: Vec<GhReviewCommentNode>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewCommentNode {
    id: String,
    author: Option<GhActor>,
    path: String,
    line: Option<u64>,
    body: String,
    url: String,
}

#[derive(Debug, serde::Deserialize)]
struct GhActor {
    login: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum GhIssueCommentsResponse {
    Paged(Vec<Vec<GhIssueCommentNode>>),
    Flat(Vec<GhIssueCommentNode>),
}

#[derive(Debug, serde::Deserialize)]
struct GhIssueCommentNode {
    id: u64,
    body: String,
    html_url: String,
    user: GhIssueCommentUser,
}

#[derive(Debug, serde::Deserialize)]
struct GhIssueCommentUser {
    login: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhIssueListItem {
    number: u64,
    title: String,
    body: Option<String>,
    url: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrBaseRefResponse {
    base_ref_name: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhRepoViewResponse {
    default_branch_ref: Option<GhRepoDefaultBranchRef>,
}

#[derive(Debug, serde::Deserialize)]
struct GhRepoDefaultBranchRef {
    name: String,
}

fn parse_repo_owner_and_name(repo: &str) -> anyhow::Result<(String, String)> {
    let Some((owner, name)) = repo.split_once('/') else {
        anyhow::bail!("Invalid --repo value `{repo}`. Expected `owner/name`.");
    };
    if owner.is_empty() || name.is_empty() || name.contains('/') {
        anyhow::bail!("Invalid --repo value `{repo}`. Expected `owner/name`.");
    }
    Ok((owner.to_string(), name.to_string()))
}

fn process_mode_issues_watch(args: ProcessIssuesWatchArgs) -> anyhow::Result<()> {
    parse_repo_owner_and_name(&args.repo)?;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let run_dir = std::path::PathBuf::from(".process/runs").join(&run_id);
    std::fs::create_dir_all(&run_dir)?;
    let fetched_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    let issues =
        fetch_open_issues_by_label(&args.repo, &args.label, args.limit).with_context(|| {
            format!(
                "Failed to fetch open issues for {} with label {}",
                args.repo, args.label
            )
        })?;
    let open_issues = issues
        .iter()
        .map(|issue| ProcessWatchIssue {
            number: issue.number,
            title: issue.title.clone(),
            url: issue.url.clone(),
        })
        .collect::<Vec<_>>();
    let artifact = ProcessIssuesWatchArtifact {
        fetched_at,
        repo: args.repo.clone(),
        label: args.label.clone(),
        suggested_actions: suggest_issue_watch_actions(&open_issues),
        open_issues,
    };
    std::fs::write(
        run_dir.join("issues-watch.json"),
        serde_json::to_string_pretty(&artifact)?,
    )?;

    if args.act {
        let quick_fix_worktree_root = run_dir.join("quick-fix-worktrees");
        let quick_fix_root_error =
            std::fs::create_dir_all(&quick_fix_worktree_root)
                .err()
                .map(|err| {
                    format!(
                        "Unable to prepare quick-fix worktree directory {}: {err}",
                        quick_fix_worktree_root.display()
                    )
                });
        let quick_fix_base_sha = current_git_head_sha();
        let quick_fix_pr_base_branch = match fetch_repo_default_branch_name(&args.repo) {
            Ok(Some(branch)) => branch,
            Ok(None) => "main".to_string(),
            Err(err) => {
                eprintln!(
                    "Warning: unable to determine default branch for {}: {err}; using `main` for follow-up PRs.",
                    args.repo
                );
                "main".to_string()
            }
        };

        let mut issue_actions = Vec::new();
        for issue in &issues {
            let decision = classify_issue_watch_triage(issue);
            let mut action = ProcessIssuesWatchActIssueAction {
                issue_number: issue.number,
                issue_url: issue.url.clone(),
                decision,
                attempted: false,
                success: false,
                branch: None,
                commit_sha: None,
                commit_url: None,
                pr_url: None,
                pr_number: None,
                update_comment_url: None,
                error: None,
            };

            match decision {
                IssueWatchDecision::NeedsManual => {
                    let reason = "Triaged as needs_manual for human follow-up (scope or risk is non-trivial).";
                    let body = format_issue_watch_manual_follow_up_comment(reason);
                    match post_issue_watch_update_comment(&args.repo, issue.number, &body) {
                        Ok(comment_url) => {
                            action.update_comment_url = comment_url;
                            action.error = Some(reason.to_string());
                        }
                        Err(err) => {
                            action.error = Some(format!(
                                "{reason} Failed to post issue update comment: {err}"
                            ));
                        }
                    }
                }
                IssueWatchDecision::QuickFix => {
                    let execution = if let Some(err) = &quick_fix_root_error {
                        QuickFixExecutionResult {
                            attempted: false,
                            success: false,
                            summary: None,
                            error: Some(err.clone()),
                            files: Vec::new(),
                            verification: None,
                            branch_name: None,
                            commit_sha: None,
                            commit_url: None,
                            pushed: None,
                            remote_branch: None,
                            follow_up_pr_url: None,
                            follow_up_pr_number: None,
                            push_error: None,
                            pr_error: None,
                        }
                    } else if let Ok(base_sha) = &quick_fix_base_sha {
                        run_issue_watch_quick_fix_in_isolated_branch(
                            &args.repo,
                            issue,
                            base_sha,
                            &quick_fix_worktree_root,
                            &run_id,
                            &quick_fix_pr_base_branch,
                        )
                    } else {
                        QuickFixExecutionResult {
                            attempted: false,
                            success: false,
                            summary: None,
                            error: Some(format!(
                                "Unable to read current git HEAD for quick-fix branching: {}",
                                quick_fix_base_sha.as_ref().err().map_or_else(
                                    || "unknown error".to_string(),
                                    ToString::to_string
                                )
                            )),
                            files: Vec::new(),
                            verification: None,
                            branch_name: None,
                            commit_sha: None,
                            commit_url: None,
                            pushed: None,
                            remote_branch: None,
                            follow_up_pr_url: None,
                            follow_up_pr_number: None,
                            push_error: None,
                            pr_error: None,
                        }
                    };

                    action.attempted = execution.attempted;
                    action.success = execution.success;
                    action.branch = execution.branch_name.clone();
                    action.commit_sha = execution.commit_sha.clone();
                    action.commit_url = execution.commit_url.clone();
                    action.pr_url = execution.follow_up_pr_url.clone();
                    action.pr_number = execution.follow_up_pr_number;
                    action.error = execution.error.clone();

                    if execution.success {
                        let body = format_issue_watch_success_comment(
                            execution.summary.as_deref(),
                            execution.follow_up_pr_url.as_deref(),
                            execution.follow_up_pr_number,
                            execution.commit_url.as_deref(),
                        );
                        match post_issue_watch_update_comment(&args.repo, issue.number, &body) {
                            Ok(comment_url) => {
                                action.update_comment_url = comment_url;
                            }
                            Err(err) => {
                                action.error = Some(format!(
                                    "Quick fix completed, but failed to post issue update comment: {err}"
                                ));
                            }
                        }
                    } else {
                        let reason = action
                            .error
                            .as_deref()
                            .unwrap_or("Quick-fix attempt failed for an unknown reason.");
                        let body = format_issue_watch_manual_follow_up_comment(reason);
                        match post_issue_watch_update_comment(&args.repo, issue.number, &body) {
                            Ok(comment_url) => {
                                action.update_comment_url = comment_url;
                            }
                            Err(err) => {
                                action.error = Some(format!(
                                    "{reason} Failed to post issue update comment: {err}"
                                ));
                            }
                        }
                    }
                }
            }
            issue_actions.push(action);
        }

        let act_artifact = ProcessIssuesWatchActArtifact {
            fetched_at,
            repo: args.repo.clone(),
            label: args.label.clone(),
            issue_actions,
        };
        std::fs::write(
            run_dir.join("issues-watch-act.json"),
            serde_json::to_string_pretty(&act_artifact)?,
        )?;

        let success_count = act_artifact
            .issue_actions
            .iter()
            .filter(|action| action.success)
            .count();
        let attempted_count = act_artifact
            .issue_actions
            .iter()
            .filter(|action| action.attempted)
            .count();
        println!("Issues watch action run created: {run_id}");
        println!("Target: {} [{}]", args.repo, args.label);
        println!("Open issues fetched: {}", artifact.open_issues.len());
        println!("Quick-fix attempted: {attempted_count}");
        println!("Quick-fix success: {success_count}");
        println!(
            "Artifact: {}",
            run_dir.join("issues-watch-act.json").display()
        );
        return Ok(());
    }

    println!("Issues watch run created: {run_id}");
    println!("Target: {} [{}]", args.repo, args.label);
    println!("Open issues fetched: {}", artifact.open_issues.len());
    println!("Artifact: {}", run_dir.join("issues-watch.json").display());
    Ok(())
}

fn fetch_unresolved_review_comments(
    owner: &str,
    name: &str,
    pr: u64,
) -> anyhow::Result<Vec<UnresolvedReviewComment>> {
    const GRAPHQL_QUERY: &str = r#"query($owner: String!, $name: String!, $pr: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $pr) {
      reviewThreads(first: 100, after: $cursor) {
        nodes {
          id
          isResolved
          comments(first: 100) {
            nodes {
              id
              author { login }
              path
              line
              body
              url
            }
          }
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }
  }
}"#;

    let mut unresolved_comments = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let mut args = vec![
            "api".to_string(),
            "graphql".to_string(),
            "-f".to_string(),
            format!("query={GRAPHQL_QUERY}"),
            "-F".to_string(),
            format!("owner={owner}"),
            "-F".to_string(),
            format!("name={name}"),
            "-F".to_string(),
            format!("pr={pr}"),
        ];
        if let Some(cursor_value) = cursor.as_ref() {
            args.push("-F".to_string());
            args.push(format!("cursor={cursor_value}"));
        }

        let page_json = run_gh_json_command(&args)?;
        let parsed_page = parse_unresolved_review_comment_page(page_json)?;
        unresolved_comments.extend(parsed_page.comments);

        if !parsed_page.has_next_page {
            break;
        }
        let Some(next_cursor) = parsed_page.end_cursor else {
            anyhow::bail!(
                "GitHub API returned pagination without a cursor for unresolved review comments."
            );
        };
        cursor = Some(next_cursor);
    }

    Ok(unresolved_comments)
}

fn fetch_issue_comments(repo: &str, pr: u64) -> anyhow::Result<Vec<OpenIssueComment>> {
    let args = vec![
        "api".to_string(),
        format!("repos/{repo}/issues/{pr}/comments"),
        "--paginate".to_string(),
        "--slurp".to_string(),
    ];
    let issue_json = run_gh_json_command(&args)?;
    parse_issue_comments(issue_json)
}

fn fetch_open_issues_by_label(
    repo: &str,
    label: &str,
    limit: u32,
) -> anyhow::Result<Vec<ProcessWatchIssueCandidate>> {
    let args = vec![
        "issue".to_string(),
        "list".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        "--state".to_string(),
        "open".to_string(),
        "--label".to_string(),
        label.to_string(),
        "--limit".to_string(),
        limit.to_string(),
        "--json".to_string(),
        "number,title,body,url".to_string(),
    ];
    let issues_json = run_gh_json_command(&args)?;
    let raw_items: Vec<GhIssueListItem> = serde_json::from_value(issues_json)
        .context("Failed to parse GitHub issue list response")?;
    Ok(raw_items
        .into_iter()
        .map(|issue| ProcessWatchIssueCandidate {
            number: issue.number,
            title: issue.title,
            body: issue.body.unwrap_or_default(),
            url: issue.url,
        })
        .collect())
}

fn fetch_repo_default_branch_name(repo: &str) -> anyhow::Result<Option<String>> {
    let args = vec![
        "repo".to_string(),
        "view".to_string(),
        repo.to_string(),
        "--json".to_string(),
        "defaultBranchRef".to_string(),
    ];
    let json = run_gh_json_command(&args)?;
    let parsed: GhRepoViewResponse = serde_json::from_value(json)
        .context("Failed to parse repository default branch response")?;
    Ok(parsed
        .default_branch_ref
        .map(|branch| branch.name.trim().to_string())
        .filter(|branch| !branch.is_empty()))
}

fn fetch_pr_base_branch_name(repo: &str, pr: u64) -> anyhow::Result<Option<String>> {
    let args = vec![
        "pr".to_string(),
        "view".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        pr.to_string(),
        "--json".to_string(),
        "baseRefName".to_string(),
    ];
    let json = run_gh_json_command(&args)?;
    let parsed: GhPrBaseRefResponse =
        serde_json::from_value(json).context("Failed to parse PR base branch response")?;
    if parsed.base_ref_name.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(parsed.base_ref_name))
}

fn resolve_review_thread(thread_id: &str) -> anyhow::Result<()> {
    const GRAPHQL_MUTATION: &str = r#"mutation($threadId: ID!) {
  resolveReviewThread(input: {threadId: $threadId}) {
    thread {
      id
      isResolved
    }
  }
}"#;
    let args = vec![
        "api".to_string(),
        "graphql".to_string(),
        "-f".to_string(),
        format!("query={GRAPHQL_MUTATION}"),
        "-F".to_string(),
        format!("threadId={thread_id}"),
    ];
    let response = run_gh_json_command(&args)?;
    parse_resolve_review_thread_response(response)
}

fn run_gh_json_command(args: &[String]) -> anyhow::Result<serde_json::Value> {
    let output = std::process::Command::new("gh")
        .args(args)
        .output()
        .map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                anyhow::anyhow!(
                    "GitHub CLI (`gh`) is not available in PATH. Install from https://cli.github.com/ and run `gh auth login`."
                )
            } else {
                anyhow::anyhow!("Failed to start `gh`: {err}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "no output captured".to_string()
        };
        anyhow::bail!(
            "`gh {}` failed with status {}: {}\nCheck `gh auth status` and verify repo/PR access.",
            args.join(" "),
            output.status,
            details
        );
    }

    serde_json::from_slice(&output.stdout).context("`gh` returned invalid JSON output")
}

fn run_gh_text_command(args: &[String]) -> anyhow::Result<String> {
    let output = std::process::Command::new("gh")
        .args(args)
        .output()
        .map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                anyhow::anyhow!(
                    "GitHub CLI (`gh`) is not available in PATH. Install from https://cli.github.com/ and run `gh auth login`."
                )
            } else {
                anyhow::anyhow!("Failed to start `gh`: {err}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "no output captured".to_string()
        };
        anyhow::bail!(
            "`gh {}` failed with status {}: {}\nCheck `gh auth status` and verify repo access.",
            args.join(" "),
            output.status,
            details
        );
    }

    let stdout = String::from_utf8(output.stdout).context("`gh` returned non-UTF8 output")?;
    Ok(stdout.trim().to_string())
}

struct UnresolvedReviewCommentPage {
    comments: Vec<UnresolvedReviewComment>,
    has_next_page: bool,
    end_cursor: Option<String>,
}

fn parse_unresolved_review_comment_page(
    page_json: serde_json::Value,
) -> anyhow::Result<UnresolvedReviewCommentPage> {
    let response: GhGraphQlResponse =
        serde_json::from_value(page_json).context("Failed to parse GitHub GraphQL response")?;

    if let Some(errors) = response.errors
        && !errors.is_empty()
    {
        let messages = errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        anyhow::bail!("GitHub GraphQL returned errors: {messages}");
    }

    let review_threads = response
        .data
        .and_then(|data| data.repository)
        .and_then(|repo| repo.pull_request)
        .map(|pr| pr.review_threads)
        .ok_or_else(|| {
            anyhow::anyhow!("GitHub GraphQL response did not include pull request review threads.")
        })?;

    let comments = review_threads
        .nodes
        .into_iter()
        .filter(|thread| !thread.is_resolved)
        .flat_map(|thread| {
            let review_thread_id = thread.id;
            thread
                .comments
                .nodes
                .into_iter()
                .map(move |comment| UnresolvedReviewComment {
                    id: comment.id,
                    review_thread_id: Some(review_thread_id.clone()),
                    author: comment
                        .author
                        .map_or_else(|| "unknown".to_string(), |author| author.login),
                    path: comment.path,
                    line: comment.line,
                    body: comment.body,
                    url: comment.url,
                })
        })
        .collect();

    Ok(UnresolvedReviewCommentPage {
        comments,
        has_next_page: review_threads.page_info.has_next_page,
        end_cursor: review_threads.page_info.end_cursor,
    })
}

fn parse_resolve_review_thread_response(response_json: serde_json::Value) -> anyhow::Result<()> {
    let response: GhResolveReviewThreadResponse = serde_json::from_value(response_json)
        .context("Failed to parse GitHub GraphQL resolveReviewThread response")?;

    if let Some(errors) = response.errors
        && !errors.is_empty()
    {
        let messages = errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ");
        anyhow::bail!("GitHub GraphQL returned errors: {messages}");
    }

    let thread = response
        .data
        .and_then(|data| data.resolve_review_thread)
        .and_then(|payload| payload.thread)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "GitHub GraphQL response did not include resolveReviewThread.thread data."
            )
        })?;
    if !thread.is_resolved {
        anyhow::bail!(
            "GitHub GraphQL resolveReviewThread returned unresolved thread {}.",
            thread.id
        );
    }
    Ok(())
}

fn parse_issue_comments(issue_json: serde_json::Value) -> anyhow::Result<Vec<OpenIssueComment>> {
    let response: GhIssueCommentsResponse = serde_json::from_value(issue_json)
        .context("Failed to parse GitHub issue comments response")?;

    let comments = match response {
        GhIssueCommentsResponse::Paged(pages) => pages.into_iter().flatten().collect(),
        GhIssueCommentsResponse::Flat(items) => items,
    };

    Ok(comments
        .into_iter()
        .map(|comment| OpenIssueComment {
            id: comment.id.to_string(),
            author: comment.user.login,
            body: comment.body,
            url: comment.html_url,
        })
        .collect())
}

fn classify_comment_triage(body: &str) -> TriageDecision {
    let normalized = body.to_ascii_lowercase();
    if [
        "follow-up",
        "follow up",
        "separate issue",
        "tracking issue",
        "out of scope",
        "later",
        "future work",
        "tech debt",
        "defer",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return TriageDecision::NeedsIssue;
    }

    if [
        "nit",
        "typo",
        "rename",
        "format",
        "formatting",
        "small fix",
        "quick fix",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
    {
        return TriageDecision::QuickFix;
    }

    if normalized.contains('?')
        || normalized.starts_with("why ")
        || normalized.starts_with("what ")
        || normalized.starts_with("can ")
        || normalized.starts_with("could ")
    {
        return TriageDecision::Question;
    }

    TriageDecision::QuickFix
}

fn classify_issue_watch_triage(issue: &ProcessWatchIssueCandidate) -> IssueWatchDecision {
    let combined = format!("{} {}", issue.title, issue.body).to_ascii_lowercase();
    if [
        "typo",
        "docs",
        "documentation",
        "readme",
        "format",
        "formatting",
        "lint",
        "rename",
        "nit",
        "small fix",
        "quick fix",
        "spelling",
        "comment",
    ]
    .iter()
    .any(|needle| combined.contains(needle))
    {
        return IssueWatchDecision::QuickFix;
    }
    IssueWatchDecision::NeedsManual
}

fn run_issue_watch_quick_fix_subprocess(
    issue: &ProcessWatchIssueCandidate,
    work_dir: &std::path::Path,
) -> QuickFixExecutionResult {
    let prompt = build_issue_watch_quick_fix_prompt(issue);
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            return QuickFixExecutionResult {
                attempted: false,
                success: false,
                summary: None,
                error: Some(format!(
                    "Unable to locate current executable for issue quick-fix run: {err}"
                )),
                files: Vec::new(),
                verification: None,
                branch_name: None,
                commit_sha: None,
                commit_url: None,
                pushed: None,
                remote_branch: None,
                follow_up_pr_url: None,
                follow_up_pr_number: None,
                push_error: None,
                pr_error: None,
            };
        }
    };
    let output = match std::process::Command::new(executable)
        .args(["exec", "--skip-git-repo-check", "--full-auto", &prompt])
        .current_dir(work_dir)
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            let error = if err.kind() == ErrorKind::NotFound {
                "Unable to execute current binary for issue quick-fix run.".to_string()
            } else {
                format!("Failed to launch issue quick-fix subprocess: {err}")
            };
            return QuickFixExecutionResult {
                attempted: true,
                success: false,
                summary: None,
                error: Some(error),
                files: Vec::new(),
                verification: None,
                branch_name: None,
                commit_sha: None,
                commit_url: None,
                pushed: None,
                remote_branch: None,
                follow_up_pr_url: None,
                follow_up_pr_number: None,
                push_error: None,
                pr_error: None,
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let parsed = parse_quick_fix_output(&stdout);
    let summary = parsed
        .summary
        .or_else(|| first_non_empty_line(&stdout))
        .or_else(|| first_non_empty_line(&stderr));

    if output.status.success() {
        return QuickFixExecutionResult {
            attempted: true,
            success: true,
            summary,
            error: None,
            files: parsed.files,
            verification: parsed.verification,
            branch_name: None,
            commit_sha: None,
            commit_url: None,
            pushed: None,
            remote_branch: None,
            follow_up_pr_url: None,
            follow_up_pr_number: None,
            push_error: None,
            pr_error: None,
        };
    }

    let status = output.status.code().map_or_else(
        || "terminated by signal".to_string(),
        |code| code.to_string(),
    );
    let detail = first_non_empty_line(&stderr)
        .or_else(|| first_non_empty_line(&stdout))
        .unwrap_or_else(|| "subprocess returned no output".to_string());
    QuickFixExecutionResult {
        attempted: true,
        success: false,
        summary,
        error: Some(format!(
            "Issue quick-fix subprocess failed (status {status}): {detail}"
        )),
        files: parsed.files,
        verification: parsed.verification,
        branch_name: None,
        commit_sha: None,
        commit_url: None,
        pushed: None,
        remote_branch: None,
        follow_up_pr_url: None,
        follow_up_pr_number: None,
        push_error: None,
        pr_error: None,
    }
}

fn run_issue_watch_quick_fix_in_isolated_branch(
    repo: &str,
    issue: &ProcessWatchIssueCandidate,
    base_sha: &str,
    worktree_root: &std::path::Path,
    run_id: &str,
    follow_up_pr_base_branch: &str,
) -> QuickFixExecutionResult {
    let branch_name = issue_watch_quick_fix_branch_name(issue.number, run_id);
    let worktree_dir = worktree_root.join(quick_fix_worktree_dir_name(&branch_name));
    if let Err(err) = create_quick_fix_worktree(&worktree_dir, &branch_name, base_sha) {
        return QuickFixExecutionResult {
            attempted: false,
            success: false,
            summary: None,
            error: Some(format!(
                "Unable to create isolated worktree for issue #{issue_number}: {err}",
                issue_number = issue.number
            )),
            files: Vec::new(),
            verification: None,
            branch_name: Some(branch_name),
            commit_sha: None,
            commit_url: None,
            pushed: None,
            remote_branch: None,
            follow_up_pr_url: None,
            follow_up_pr_number: None,
            push_error: None,
            pr_error: None,
        };
    }

    let mut execution = run_issue_watch_quick_fix_subprocess(issue, &worktree_dir);
    execution.branch_name = Some(branch_name.clone());
    if !execution.success {
        return execution;
    }

    match commit_issue_watch_quick_fix_changes(repo, issue, &branch_name, &worktree_dir) {
        Ok(metadata) => {
            execution.branch_name = Some(metadata.branch_name);
            execution.commit_sha = Some(metadata.commit_sha);
            execution.commit_url = Some(metadata.commit_url);
        }
        Err(err) => {
            execution.success = false;
            execution.error = Some(format!(
                "Issue quick-fix commit step failed for issue #{issue_number}: {err}",
                issue_number = issue.number
            ));
            execution.commit_sha = None;
            execution.commit_url = None;
            return execution;
        }
    }

    if let Some(push_error) = push_quick_fix_branch(&worktree_dir, &branch_name).err() {
        execution.success = false;
        execution.pushed = Some(false);
        execution.remote_branch = Some(branch_name);
        execution.push_error = Some(push_error.to_string());
        execution.error = Some(format!(
            "Issue quick-fix push failed for issue #{issue_number}: {push_error}",
            issue_number = issue.number
        ));
        return execution;
    }

    execution.pushed = Some(true);
    execution.remote_branch = Some(branch_name.clone());
    match create_issue_watch_follow_up_pr(
        repo,
        follow_up_pr_base_branch,
        issue,
        &branch_name,
        execution.commit_url.as_deref(),
    ) {
        Ok((pr_url, pr_number)) => {
            execution.follow_up_pr_url = Some(pr_url);
            execution.follow_up_pr_number = pr_number;
        }
        Err(err) => {
            execution.success = false;
            execution.pr_error = Some(err.to_string());
            execution.error = Some(format!(
                "Issue quick-fix PR creation failed for issue #{issue_number}: {err}",
                issue_number = issue.number
            ));
        }
    }

    execution
}

fn run_quick_fix_subprocess(
    repo: &str,
    pr: u64,
    item: &ProcessCommentTriageItem,
    work_dir: &std::path::Path,
) -> QuickFixExecutionResult {
    let prompt = build_quick_fix_prompt(repo, pr, item);
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(err) => {
            return QuickFixExecutionResult {
                attempted: false,
                success: false,
                summary: None,
                error: Some(format!(
                    "Unable to locate current executable for quick-fix run: {err}"
                )),
                files: Vec::new(),
                verification: None,
                branch_name: None,
                commit_sha: None,
                commit_url: None,
                pushed: None,
                remote_branch: None,
                follow_up_pr_url: None,
                follow_up_pr_number: None,
                push_error: None,
                pr_error: None,
            };
        }
    };
    let output = match std::process::Command::new(executable)
        .args(["exec", "--skip-git-repo-check", "--full-auto", &prompt])
        .current_dir(work_dir)
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            let error = if err.kind() == ErrorKind::NotFound {
                "Unable to execute current binary for quick-fix run.".to_string()
            } else {
                format!("Failed to launch quick-fix subprocess: {err}")
            };
            return QuickFixExecutionResult {
                attempted: true,
                success: false,
                summary: None,
                error: Some(error),
                files: Vec::new(),
                verification: None,
                branch_name: None,
                commit_sha: None,
                commit_url: None,
                pushed: None,
                remote_branch: None,
                follow_up_pr_url: None,
                follow_up_pr_number: None,
                push_error: None,
                pr_error: None,
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let parsed = parse_quick_fix_output(&stdout);
    let summary = parsed
        .summary
        .or_else(|| first_non_empty_line(&stdout))
        .or_else(|| first_non_empty_line(&stderr));

    if output.status.success() {
        return QuickFixExecutionResult {
            attempted: true,
            success: true,
            summary,
            error: None,
            files: parsed.files,
            verification: parsed.verification,
            branch_name: None,
            commit_sha: None,
            commit_url: None,
            pushed: None,
            remote_branch: None,
            follow_up_pr_url: None,
            follow_up_pr_number: None,
            push_error: None,
            pr_error: None,
        };
    }

    let status = output.status.code().map_or_else(
        || "terminated by signal".to_string(),
        |code| code.to_string(),
    );
    let detail = first_non_empty_line(&stderr)
        .or_else(|| first_non_empty_line(&stdout))
        .unwrap_or_else(|| "subprocess returned no output".to_string());
    QuickFixExecutionResult {
        attempted: true,
        success: false,
        summary,
        error: Some(format!(
            "Quick-fix subprocess failed (status {status}): {detail}"
        )),
        files: parsed.files,
        verification: parsed.verification,
        branch_name: None,
        commit_sha: None,
        commit_url: None,
        pushed: None,
        remote_branch: None,
        follow_up_pr_url: None,
        follow_up_pr_number: None,
        push_error: None,
        pr_error: None,
    }
}

#[derive(Debug, Clone)]
struct QuickFixCommitMetadata {
    branch_name: String,
    commit_sha: String,
    commit_url: String,
}

fn run_quick_fix_item_in_isolated_branch(
    repo: &str,
    pr: u64,
    item: &ProcessCommentTriageItem,
    base_sha: &str,
    worktree_root: &std::path::Path,
    follow_up_pr_base_branch: &str,
) -> QuickFixExecutionResult {
    let branch_name = quick_fix_branch_name(pr, &item.comment_id);
    let worktree_dir = worktree_root.join(quick_fix_worktree_dir_name(&branch_name));
    if let Err(err) = create_quick_fix_worktree(&worktree_dir, &branch_name, base_sha) {
        return QuickFixExecutionResult {
            attempted: false,
            success: false,
            summary: None,
            error: Some(format!(
                "Unable to create isolated worktree for quick-fix item {comment_id}: {err}",
                comment_id = item.comment_id
            )),
            files: Vec::new(),
            verification: None,
            branch_name: Some(branch_name),
            commit_sha: None,
            commit_url: None,
            pushed: None,
            remote_branch: None,
            follow_up_pr_url: None,
            follow_up_pr_number: None,
            push_error: None,
            pr_error: None,
        };
    }

    let mut execution = run_quick_fix_subprocess(repo, pr, item, &worktree_dir);
    execution.branch_name = Some(branch_name.clone());
    if !execution.success {
        return execution;
    }

    match commit_quick_fix_changes(repo, pr, item, &branch_name, &worktree_dir) {
        Ok(metadata) => {
            execution.branch_name = Some(metadata.branch_name);
            execution.commit_sha = Some(metadata.commit_sha);
            execution.commit_url = Some(metadata.commit_url);
        }
        Err(err) => {
            execution.success = false;
            execution.error = Some(format!(
                "Quick-fix commit step failed for comment {comment_id}: {err}",
                comment_id = item.comment_id
            ));
            execution.commit_sha = None;
            execution.commit_url = None;
            return execution;
        }
    }

    if let Some(push_error) = push_quick_fix_branch(&worktree_dir, &branch_name).err() {
        execution.pushed = Some(false);
        execution.remote_branch = Some(branch_name);
        execution.push_error = Some(push_error.to_string());
        execution.pr_error = Some("Skipped PR creation because branch push failed.".to_string());
        return execution;
    }

    execution.pushed = Some(true);
    execution.remote_branch = Some(branch_name.clone());
    let pr_result = create_quick_fix_follow_up_pr(
        repo,
        pr,
        follow_up_pr_base_branch,
        item,
        &branch_name,
        execution.commit_url.as_deref(),
    );
    match pr_result {
        Ok((pr_url, pr_number)) => {
            execution.follow_up_pr_url = Some(pr_url);
            execution.follow_up_pr_number = pr_number;
        }
        Err(err) => {
            execution.pr_error = Some(err.to_string());
        }
    }
    execution
}

fn push_quick_fix_branch(worktree_dir: &std::path::Path, branch_name: &str) -> anyhow::Result<()> {
    let args = vec![
        "-C".to_string(),
        worktree_dir.display().to_string(),
        "push".to_string(),
        "-u".to_string(),
        "origin".to_string(),
        branch_name.to_string(),
    ];
    run_git_text_command(&args).map(|_| ())
}

fn create_quick_fix_follow_up_pr(
    repo: &str,
    pr: u64,
    base_branch: &str,
    item: &ProcessCommentTriageItem,
    head_branch: &str,
    commit_url: Option<&str>,
) -> anyhow::Result<(String, Option<u64>)> {
    let title = quick_fix_commit_message(pr, &item.comment_id);
    let body = format_quick_fix_follow_up_pr_body(repo, pr, item, commit_url);
    let body_file = write_temp_markdown_file("codex-follow-up-pr", &body)?;
    let args = vec![
        "pr".to_string(),
        "create".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        "--base".to_string(),
        base_branch.to_string(),
        "--head".to_string(),
        head_branch.to_string(),
        "--title".to_string(),
        title,
        "--body-file".to_string(),
        body_file.display().to_string(),
    ];
    let create_output = run_gh_text_command(&args);
    let _ = std::fs::remove_file(&body_file);
    let create_output = create_output?;
    let parsed = parse_gh_pr_create_output(&create_output);
    let Some(pr_url) = parsed.url else {
        anyhow::bail!("`gh pr create` succeeded but did not return a PR URL: {create_output}");
    };
    Ok((pr_url, parsed.number))
}

fn create_issue_watch_follow_up_pr(
    repo: &str,
    base_branch: &str,
    issue: &ProcessWatchIssueCandidate,
    head_branch: &str,
    commit_url: Option<&str>,
) -> anyhow::Result<(String, Option<u64>)> {
    let title = issue_watch_quick_fix_commit_message(issue.number);
    let body = format_issue_watch_follow_up_pr_body(repo, issue, commit_url);
    let body_file = write_temp_markdown_file("codex-issues-watch-follow-up-pr", &body)?;
    let args = vec![
        "pr".to_string(),
        "create".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        "--base".to_string(),
        base_branch.to_string(),
        "--head".to_string(),
        head_branch.to_string(),
        "--title".to_string(),
        title,
        "--body-file".to_string(),
        body_file.display().to_string(),
    ];
    let create_output = run_gh_text_command(&args);
    let _ = std::fs::remove_file(&body_file);
    let create_output = create_output?;
    let parsed = parse_gh_pr_create_output(&create_output);
    let Some(pr_url) = parsed.url else {
        anyhow::bail!("`gh pr create` succeeded but did not return a PR URL: {create_output}");
    };
    Ok((pr_url, parsed.number))
}

fn create_quick_fix_worktree(
    worktree_dir: &std::path::Path,
    branch_name: &str,
    base_sha: &str,
) -> anyhow::Result<()> {
    if let Some(parent) = worktree_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let args = vec![
        "worktree".to_string(),
        "add".to_string(),
        "-b".to_string(),
        branch_name.to_string(),
        worktree_dir.display().to_string(),
        base_sha.to_string(),
    ];
    run_git_text_command(&args).map(|_| ())
}

fn commit_quick_fix_changes(
    repo: &str,
    pr: u64,
    item: &ProcessCommentTriageItem,
    branch_name: &str,
    worktree_dir: &std::path::Path,
) -> anyhow::Result<QuickFixCommitMetadata> {
    let add_args = vec![
        "-C".to_string(),
        worktree_dir.display().to_string(),
        "add".to_string(),
        "-A".to_string(),
    ];
    run_git_text_command(&add_args)?;

    let has_changes = git_has_staged_changes(worktree_dir)?;
    if !has_changes {
        anyhow::bail!("no changes were produced to commit");
    }

    let commit_args = vec![
        "-C".to_string(),
        worktree_dir.display().to_string(),
        "commit".to_string(),
        "-m".to_string(),
        quick_fix_commit_message(pr, &item.comment_id),
    ];
    run_git_text_command(&commit_args)?;

    let sha_args = vec![
        "-C".to_string(),
        worktree_dir.display().to_string(),
        "rev-parse".to_string(),
        "HEAD".to_string(),
    ];
    let commit_sha = run_git_text_command(&sha_args)?;

    Ok(QuickFixCommitMetadata {
        branch_name: branch_name.to_string(),
        commit_url: quick_fix_commit_url(repo, &commit_sha),
        commit_sha,
    })
}

fn commit_issue_watch_quick_fix_changes(
    repo: &str,
    issue: &ProcessWatchIssueCandidate,
    branch_name: &str,
    worktree_dir: &std::path::Path,
) -> anyhow::Result<QuickFixCommitMetadata> {
    let add_args = vec![
        "-C".to_string(),
        worktree_dir.display().to_string(),
        "add".to_string(),
        "-A".to_string(),
    ];
    run_git_text_command(&add_args)?;

    let has_changes = git_has_staged_changes(worktree_dir)?;
    if !has_changes {
        anyhow::bail!("no changes were produced to commit");
    }

    let commit_args = vec![
        "-C".to_string(),
        worktree_dir.display().to_string(),
        "commit".to_string(),
        "-m".to_string(),
        issue_watch_quick_fix_commit_message(issue.number),
    ];
    run_git_text_command(&commit_args)?;

    let sha_args = vec![
        "-C".to_string(),
        worktree_dir.display().to_string(),
        "rev-parse".to_string(),
        "HEAD".to_string(),
    ];
    let commit_sha = run_git_text_command(&sha_args)?;

    Ok(QuickFixCommitMetadata {
        branch_name: branch_name.to_string(),
        commit_url: quick_fix_commit_url(repo, &commit_sha),
        commit_sha,
    })
}

fn current_git_head_sha() -> anyhow::Result<String> {
    let args = vec!["rev-parse".to_string(), "HEAD".to_string()];
    run_git_text_command(&args)
}

fn git_has_staged_changes(worktree_dir: &std::path::Path) -> anyhow::Result<bool> {
    let worktree = worktree_dir.display().to_string();
    let output = std::process::Command::new("git")
        .args(["-C", &worktree, "diff", "--cached", "--quiet"])
        .output()
        .map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                anyhow::anyhow!(
                    "git is not available in PATH. Install git before running process quick-fix actions."
                )
            } else {
                anyhow::anyhow!("Failed to start `git`: {err}")
            }
        })?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let details = if !stderr.is_empty() { stderr } else { stdout };
            anyhow::bail!("`git diff --cached --quiet` failed: {}", details);
        }
    }
}

fn run_git_text_command(args: &[String]) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .output()
        .map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                anyhow::anyhow!("git is not available in PATH.")
            } else {
                anyhow::anyhow!("Failed to start `git`: {err}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "no output captured".to_string()
        };
        anyhow::bail!(
            "`git {}` failed with status {}: {}",
            args.join(" "),
            output.status,
            details
        );
    }

    let stdout = String::from_utf8(output.stdout).context("`git` returned non-UTF8 output")?;
    Ok(stdout.trim().to_string())
}

fn quick_fix_commit_message(pr: u64, comment_id: &str) -> String {
    format!("process: quick fix for PR #{pr} comment {comment_id}")
}

fn issue_watch_quick_fix_commit_message(issue_number: u64) -> String {
    format!("process: quick fix for issue #{issue_number}")
}

fn quick_fix_commit_url(repo: &str, sha: &str) -> String {
    format!("https://github.com/{repo}/commit/{sha}")
}

fn quick_fix_branch_name(pr: u64, comment_id: &str) -> String {
    let short = short_comment_id_for_branch(comment_id);
    format!("process/quick-fix-pr-{pr}-{short}")
}

fn issue_watch_quick_fix_branch_name(issue_number: u64, run_id: &str) -> String {
    let suffix = run_id
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("process/quick-fix-issue-{issue_number}-{suffix}")
}

fn quick_fix_worktree_dir_name(branch_name: &str) -> String {
    branch_name.replace('/', "__")
}

fn short_comment_id_for_branch(comment_id: &str) -> String {
    let mut short = String::new();
    let mut last_was_dash = false;
    for ch in comment_id.chars() {
        if ch.is_ascii_alphanumeric() {
            short.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !short.is_empty() {
            short.push('-');
            last_was_dash = true;
        }
        if short.len() >= 24 {
            break;
        }
    }
    while short.ends_with('-') {
        short.pop();
    }
    if short.is_empty() {
        return "comment".to_string();
    }
    short
}

fn build_quick_fix_prompt(repo: &str, pr: u64, item: &ProcessCommentTriageItem) -> String {
    let trimmed_body = item.body.trim();
    let bounded_body = trimmed_body.chars().take(2_000).collect::<String>();
    format!(
        "You are applying a minimal targeted fix in this repository.\n\nRepository: {repo}\nPR number: {pr}\nComment URL: {comment_url}\nComment body:\n{comment_body}\n\nRequirements:\n- Apply the smallest safe change that addresses the comment.\n- Keep scope tight to this comment only.\n- If verification is quick, run only focused checks.\n- Return exactly these lines at the end:\nSUMMARY: <one-line summary>\nFILES: <comma-separated file paths or none>\nVERIFICATION: <short status or none>",
        comment_url = item.comment_url,
        comment_body = bounded_body,
    )
}

fn build_issue_watch_quick_fix_prompt(issue: &ProcessWatchIssueCandidate) -> String {
    let trimmed_body = issue.body.trim();
    let bounded_body = trimmed_body.chars().take(2_000).collect::<String>();
    format!(
        "You are applying a minimal targeted fix in this repository.\n\nIssue number: {issue_number}\nIssue URL: {issue_url}\nIssue title: {issue_title}\nIssue body:\n{issue_body}\n\nRequirements:\n- Apply the smallest safe change that resolves this issue.\n- Keep scope tight to this issue only.\n- If verification is quick, run only focused checks.\n- Return exactly these lines at the end:\nSUMMARY: <one-line summary>\nFILES: <comma-separated file paths or none>\nVERIFICATION: <short status or none>",
        issue_number = issue.number,
        issue_url = issue.url.as_str(),
        issue_title = issue.title.as_str(),
        issue_body = bounded_body,
    )
}

fn parse_quick_fix_output(output: &str) -> ParsedQuickFixOutput {
    let mut parsed = ParsedQuickFixOutput::default();
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(summary) = trimmed.strip_prefix("SUMMARY:") {
            let summary = summary.trim();
            if !summary.is_empty() {
                parsed.summary = Some(summary.to_string());
            }
            continue;
        }
        if let Some(files) = trimmed.strip_prefix("FILES:") {
            let files = files.trim();
            if !files.eq_ignore_ascii_case("none") && !files.is_empty() {
                parsed.files = files
                    .split(',')
                    .map(str::trim)
                    .filter(|segment| !segment.is_empty())
                    .map(ToString::to_string)
                    .collect();
            }
            continue;
        }
        if let Some(verification) = trimmed.strip_prefix("VERIFICATION:") {
            let verification = verification.trim();
            if !verification.eq_ignore_ascii_case("none") && !verification.is_empty() {
                parsed.verification = Some(verification.to_string());
            }
        }
    }
    parsed
}

fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn post_quick_fix_pr_update_comment(
    repo: &str,
    pr: u64,
    summaries: &[QuickFixSummary],
) -> anyhow::Result<Option<String>> {
    if summaries.is_empty() {
        return Ok(None);
    }

    let body = format_pr_update_comment_body(summaries);
    let body_file = write_temp_markdown_file("codex-pr-update", &body)?;
    let args = vec![
        "pr".to_string(),
        "comment".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        pr.to_string(),
        "--body-file".to_string(),
        body_file.display().to_string(),
    ];
    let result = run_gh_text_command(&args);
    let _ = std::fs::remove_file(&body_file);
    let text = result?;
    Ok(extract_first_url(&text))
}

fn post_issue_watch_update_comment(
    repo: &str,
    issue_number: u64,
    body: &str,
) -> anyhow::Result<Option<String>> {
    let body_file = write_temp_markdown_file("codex-issues-watch-update", body)?;
    let args = vec![
        "issue".to_string(),
        "comment".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        issue_number.to_string(),
        "--body-file".to_string(),
        body_file.display().to_string(),
    ];
    let result = run_gh_text_command(&args);
    let _ = std::fs::remove_file(&body_file);
    let text = result?;
    let parsed = parse_gh_issue_comment_output(&text);
    Ok(parsed.url)
}

fn format_pr_update_comment_body(summaries: &[QuickFixSummary]) -> String {
    let mut body = String::from("Quick-fix update from `codex process pr-comments --act`.\n\n");
    body.push_str("Applied items:\n");
    for item in summaries {
        let mut line = format!("- `{}`: {}", item.comment_id, item.summary);
        if !item.files.is_empty() {
            let files = item.files.join(", ");
            line.push_str(&format!(" (files: {files})"));
        }
        if let Some(verification) = &item.verification {
            line.push_str(&format!("; verification: {verification}"));
        }
        if let (Some(commit_sha), Some(commit_url)) = (&item.commit_sha, &item.commit_url) {
            line.push_str(&format!(
                "; commit: [`{}`]({commit_url})",
                short_commit_sha(commit_sha)
            ));
        }
        if let Some(pr_url) = &item.follow_up_pr_url {
            if let Some(pr_number) = item.follow_up_pr_number {
                line.push_str(&format!("; follow-up PR: [#{pr_number}]({pr_url})"));
            } else {
                line.push_str(&format!("; follow-up PR: {pr_url}"));
            }
        }
        if let Some(thread_resolved) = item.thread_resolved {
            if thread_resolved {
                line.push_str("; review thread: resolved");
            } else {
                line.push_str("; review thread: resolution failed");
                if let Some(resolve_error) = &item.thread_resolve_error {
                    line.push_str(&format!(" ({resolve_error})"));
                }
            }
        }
        body.push_str(&line);
        body.push('\n');
    }
    body
}

fn short_commit_sha(sha: &str) -> &str {
    if sha.len() > 12 {
        return &sha[..12];
    }
    sha
}

fn extract_first_url(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|token| {
        if token.starts_with("https://") || token.starts_with("http://") {
            return Some(
                token
                    .trim_end_matches(|ch: char| ",.)]".contains(ch))
                    .to_string(),
            );
        }
        None
    })
}

fn parse_pr_number_from_url(url: &str) -> Option<u64> {
    let (_, suffix) = url.split_once("/pull/")?;
    let digits = suffix
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn parse_pr_number_from_text(text: &str) -> Option<u64> {
    for token in text.split_whitespace() {
        let trimmed = token.trim_matches(|ch: char| "[](){}<>:;,.!?".contains(ch));
        if let Some(number) = trimmed.strip_prefix('#')
            && !number.is_empty()
            && number.chars().all(|ch| ch.is_ascii_digit())
            && let Ok(parsed) = number.parse::<u64>()
        {
            return Some(parsed);
        }
        if let Some((_, number)) = trimmed.split_once("/pull/") {
            let digits = number
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if !digits.is_empty()
                && let Ok(parsed) = digits.parse::<u64>()
            {
                return Some(parsed);
            }
        }
    }
    None
}

fn parse_gh_pr_create_output(text: &str) -> ParsedGhPrCreateOutput {
    let url = text
        .split_whitespace()
        .filter_map(|token| {
            if token.starts_with("https://") || token.starts_with("http://") {
                return Some(
                    token
                        .trim_end_matches(|ch: char| ",.)]".contains(ch))
                        .to_string(),
                );
            }
            None
        })
        .find(|url| url.contains("/pull/"))
        .or_else(|| extract_first_url(text));
    let number = url
        .as_deref()
        .and_then(parse_pr_number_from_url)
        .or_else(|| parse_pr_number_from_text(text));
    ParsedGhPrCreateOutput { url, number }
}

fn parse_gh_issue_comment_output(text: &str) -> ParsedGhIssueCommentOutput {
    let url = text
        .split_whitespace()
        .filter_map(|token| {
            if token.starts_with("https://") || token.starts_with("http://") {
                return Some(
                    token
                        .trim_end_matches(|ch: char| ",.)]".contains(ch))
                        .to_string(),
                );
            }
            None
        })
        .find(|candidate| candidate.contains("/issues/") && candidate.contains("#issuecomment-"));
    ParsedGhIssueCommentOutput { url }
}

fn write_temp_markdown_file(prefix: &str, body: &str) -> anyhow::Result<std::path::PathBuf> {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("{prefix}-{millis}-{pid}.md"));
    std::fs::write(&path, body)?;
    Ok(path)
}

fn to_markdown_blockquote(text: &str) -> String {
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            lines.push(">".to_string());
        } else {
            lines.push(format!("> {}", line.trim_end()));
        }
    }
    if lines.is_empty() {
        return "> (empty comment)".to_string();
    }
    lines.join("\n")
}

fn format_follow_up_issue_body(
    repo: &str,
    pr: u64,
    comment_url: &str,
    comment_body: &str,
) -> String {
    format!(
        "Created by `codex process pr-comments --act`.\n\n- Source PR: https://github.com/{repo}/pull/{pr}\n- Source comment: {comment_url}\n\nOriginal comment:\n\n{quoted}\n",
        quoted = to_markdown_blockquote(comment_body),
    )
}

fn format_quick_fix_follow_up_pr_body(
    repo: &str,
    pr: u64,
    item: &ProcessCommentTriageItem,
    commit_url: Option<&str>,
) -> String {
    let mut body = format!(
        "Created by `codex process pr-comments --act`.\n\n- Source PR: https://github.com/{repo}/pull/{pr}\n- Source comment: {comment_url}\n",
        comment_url = item.comment_url
    );
    if let Some(commit_url) = commit_url {
        body.push_str(&format!("- Quick-fix commit: {commit_url}\n"));
    }
    body.push_str("\nOriginal comment:\n\n");
    body.push_str(&to_markdown_blockquote(&item.body));
    body.push('\n');
    body
}

fn format_issue_watch_follow_up_pr_body(
    repo: &str,
    issue: &ProcessWatchIssueCandidate,
    commit_url: Option<&str>,
) -> String {
    let mut body = format!(
        "Created by `codex process issues watch --act`.\n\n- Source issue: {issue_url}\n- Repository: https://github.com/{repo}\n",
        issue_url = issue.url.as_str()
    );
    if let Some(commit_url) = commit_url {
        body.push_str(&format!("- Quick-fix commit: {commit_url}\n"));
    }
    body.push_str("\nIssue summary:\n\n");
    body.push_str(&format!(
        "- #{issue_number}: {title}\n",
        issue_number = issue.number,
        title = issue.title.as_str()
    ));
    body
}

fn format_issue_watch_success_comment(
    summary: Option<&str>,
    pr_url: Option<&str>,
    pr_number: Option<u64>,
    commit_url: Option<&str>,
) -> String {
    let mut body = String::from("Automation update from `codex process issues watch --act`.\n\n");
    if let Some(summary) = summary {
        body.push_str(&format!("- Result: {summary}\n"));
    } else {
        body.push_str("- Result: Applied targeted quick fix.\n");
    }
    if let (Some(pr_url), Some(pr_number)) = (pr_url, pr_number) {
        body.push_str(&format!("- Follow-up PR: [#{pr_number}]({pr_url})\n"));
    } else if let Some(pr_url) = pr_url {
        body.push_str(&format!("- Follow-up PR: {pr_url}\n"));
    }
    if let Some(commit_url) = commit_url {
        body.push_str(&format!("- Commit: {commit_url}\n"));
    }
    body
}

fn format_issue_watch_manual_follow_up_comment(reason: &str) -> String {
    format!(
        "Automation update from `codex process issues watch --act`.\n\nManual follow-up needed: {reason}\n"
    )
}

fn create_follow_up_issue(
    repo: &str,
    pr: u64,
    author: &str,
    comment_url: &str,
    comment_body: &str,
) -> anyhow::Result<String> {
    let mut title = format!("Follow-up from PR #{pr} comment by {author}");
    let first_line = comment_body.lines().next().unwrap_or_default().trim();
    if !first_line.is_empty() {
        let truncated = first_line.chars().take(80).collect::<String>();
        title = format!("{title}: {truncated}");
    }

    let body = format_follow_up_issue_body(repo, pr, comment_url, comment_body);
    let body_file = write_temp_markdown_file("codex-follow-up-issue", &body)?;
    let args = vec![
        "issue".to_string(),
        "create".to_string(),
        "--repo".to_string(),
        repo.to_string(),
        "--title".to_string(),
        title,
        "--body-file".to_string(),
        body_file.display().to_string(),
    ];
    let issue_url = run_gh_text_command(&args);
    let _ = std::fs::remove_file(&body_file);
    let issue_url = issue_url?;
    if let Some(url) = extract_first_url(&issue_url) {
        return Ok(url);
    }
    anyhow::bail!("`gh issue create` succeeded but did not return an issue URL: {issue_url}");
}

fn suggested_next_actions(grouped_by_type: &GroupedByType) -> Vec<String> {
    let mut actions = Vec::new();
    if grouped_by_type.unresolved_review_comments > 0 {
        actions.push("Address unresolved review comments first.".to_string());
    }
    if grouped_by_type.open_issue_comments > 0 {
        actions.push("Reply to open issue comments on the PR conversation.".to_string());
    }
    if grouped_by_type.total == 0 {
        actions.push(
            "No open comments found; proceed with final verification and merge checks.".to_string(),
        );
    } else {
        actions.push("After fixes, rerun `codex process pr-comments --repo <owner/name> --pr <number>` to confirm all comments are addressed.".to_string());
    }
    actions
}

fn suggest_issue_watch_actions(open_issues: &[ProcessWatchIssue]) -> Vec<String> {
    if open_issues.is_empty() {
        return vec!["No matching open issues found.".to_string()];
    }

    open_issues
        .iter()
        .take(5)
        .map(|issue| {
            format!(
                "Plan automation for issue #{number} ({url})",
                number = issue.number,
                url = issue.url.as_str()
            )
        })
        .collect()
}

async fn enable_feature_in_config(interactive: &TuiCli, feature: &str) -> anyhow::Result<()> {
    FeatureToggles::validate_feature(feature)?;
    let codex_home = find_codex_home()?;
    ConfigEditsBuilder::new(&codex_home)
        .with_profile(interactive.config_profile.as_deref())
        .set_feature_enabled(feature, true)
        .apply()
        .await?;
    println!("Enabled feature `{feature}` in config.toml.");
    maybe_print_under_development_feature_warning(&codex_home, interactive, feature);
    Ok(())
}

async fn disable_feature_in_config(interactive: &TuiCli, feature: &str) -> anyhow::Result<()> {
    FeatureToggles::validate_feature(feature)?;
    let codex_home = find_codex_home()?;
    ConfigEditsBuilder::new(&codex_home)
        .with_profile(interactive.config_profile.as_deref())
        .set_feature_enabled(feature, false)
        .apply()
        .await?;
    println!("Disabled feature `{feature}` in config.toml.");
    Ok(())
}

fn maybe_print_under_development_feature_warning(
    codex_home: &std::path::Path,
    interactive: &TuiCli,
    feature: &str,
) {
    if interactive.config_profile.is_some() {
        return;
    }

    let Some(spec) = codex_core::features::FEATURES
        .iter()
        .find(|spec| spec.key == feature)
    else {
        return;
    };
    if !matches!(spec.stage, Stage::UnderDevelopment) {
        return;
    }

    let config_path = codex_home.join(codex_config::CONFIG_TOML_FILE);
    eprintln!(
        "Under-development features enabled: {feature}. Under-development features are incomplete and may behave unpredictably. To suppress this warning, set `suppress_unstable_features_warning = true` in {}.",
        config_path.display()
    );
}

async fn run_debug_clear_memories_command(
    root_config_overrides: &CliConfigOverrides,
    interactive: &TuiCli,
) -> anyhow::Result<()> {
    let cli_kv_overrides = root_config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let overrides = ConfigOverrides {
        config_profile: interactive.config_profile.clone(),
        ..Default::default()
    };
    let config =
        Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides).await?;

    let state_path = state_db_path(config.sqlite_home.as_path());
    let mut cleared_state_db = false;
    if tokio::fs::try_exists(&state_path).await? {
        let state_db =
            StateRuntime::init(config.sqlite_home.clone(), config.model_provider_id.clone())
                .await?;
        state_db.reset_memory_data_for_fresh_start().await?;
        cleared_state_db = true;
    }

    let memory_root = config.codex_home.join("memories");
    let removed_memory_root = match tokio::fs::remove_dir_all(&memory_root).await {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err.into()),
    };

    let mut message = if cleared_state_db {
        format!("Cleared memory state from {}.", state_path.display())
    } else {
        format!("No state db found at {}.", state_path.display())
    };

    if removed_memory_root {
        message.push_str(&format!(" Removed {}.", memory_root.display()));
    } else {
        message.push_str(&format!(
            " No memory directory found at {}.",
            memory_root.display()
        ));
    }

    println!("{message}");

    Ok(())
}

/// Prepend root-level overrides so they have lower precedence than
/// CLI-specific ones specified after the subcommand (if any).
fn prepend_config_flags(
    subcommand_config_overrides: &mut CliConfigOverrides,
    cli_config_overrides: CliConfigOverrides,
) {
    subcommand_config_overrides
        .raw_overrides
        .splice(0..0, cli_config_overrides.raw_overrides);
}

async fn run_interactive_tui(
    mut interactive: TuiCli,
    arg0_paths: Arg0DispatchPaths,
) -> std::io::Result<AppExitInfo> {
    if let Some(prompt) = interactive.prompt.take() {
        // Normalize CRLF/CR to LF so CLI-provided text can't leak `\r` into TUI state.
        interactive.prompt = Some(prompt.replace("\r\n", "\n").replace('\r', "\n"));
    }

    let terminal_info = codex_core::terminal::terminal_info();
    if terminal_info.name == TerminalName::Dumb {
        if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
            return Ok(AppExitInfo::fatal(
                "TERM is set to \"dumb\". Refusing to start the interactive TUI because no terminal is available for a confirmation prompt (stdin/stderr is not a TTY). Run in a supported terminal or unset TERM.",
            ));
        }

        eprintln!(
            "WARNING: TERM is set to \"dumb\". Codex's interactive TUI may not work in this terminal."
        );
        if !confirm("Continue anyway? [y/N]: ")? {
            return Ok(AppExitInfo::fatal(
                "Refusing to start the interactive TUI because TERM is set to \"dumb\". Run in a supported terminal or unset TERM.",
            ));
        }
    }

    codex_tui::run_main(interactive, arg0_paths).await
}

fn confirm(prompt: &str) -> std::io::Result<bool> {
    eprintln!("{prompt}");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let answer = input.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

/// Build the final `TuiCli` for a `codex resume` invocation.
fn finalize_resume_interactive(
    mut interactive: TuiCli,
    root_config_overrides: CliConfigOverrides,
    session_id: Option<String>,
    last: bool,
    show_all: bool,
    resume_cli: TuiCli,
) -> TuiCli {
    // Start with the parsed interactive CLI so resume shares the same
    // configuration surface area as `codex` without additional flags.
    let resume_session_id = session_id;
    interactive.resume_picker = resume_session_id.is_none() && !last;
    interactive.resume_last = last;
    interactive.resume_session_id = resume_session_id;
    interactive.resume_show_all = show_all;

    // Merge resume-scoped flags and overrides with highest precedence.
    merge_interactive_cli_flags(&mut interactive, resume_cli);

    // Propagate any root-level config overrides (e.g. `-c key=value`).
    prepend_config_flags(&mut interactive.config_overrides, root_config_overrides);

    interactive
}

/// Build the final `TuiCli` for a `codex fork` invocation.
fn finalize_fork_interactive(
    mut interactive: TuiCli,
    root_config_overrides: CliConfigOverrides,
    session_id: Option<String>,
    last: bool,
    show_all: bool,
    fork_cli: TuiCli,
) -> TuiCli {
    // Start with the parsed interactive CLI so fork shares the same
    // configuration surface area as `codex` without additional flags.
    let fork_session_id = session_id;
    interactive.fork_picker = fork_session_id.is_none() && !last;
    interactive.fork_last = last;
    interactive.fork_session_id = fork_session_id;
    interactive.fork_show_all = show_all;

    // Merge fork-scoped flags and overrides with highest precedence.
    merge_interactive_cli_flags(&mut interactive, fork_cli);

    // Propagate any root-level config overrides (e.g. `-c key=value`).
    prepend_config_flags(&mut interactive.config_overrides, root_config_overrides);

    interactive
}

/// Merge flags provided to `codex resume`/`codex fork` so they take precedence over any
/// root-level flags. Only overrides fields explicitly set on the subcommand-scoped
/// CLI. Also appends `-c key=value` overrides with highest precedence.
fn merge_interactive_cli_flags(interactive: &mut TuiCli, subcommand_cli: TuiCli) {
    if let Some(model) = subcommand_cli.model {
        interactive.model = Some(model);
    }
    if subcommand_cli.oss {
        interactive.oss = true;
    }
    if let Some(profile) = subcommand_cli.config_profile {
        interactive.config_profile = Some(profile);
    }
    if let Some(sandbox) = subcommand_cli.sandbox_mode {
        interactive.sandbox_mode = Some(sandbox);
    }
    if let Some(approval) = subcommand_cli.approval_policy {
        interactive.approval_policy = Some(approval);
    }
    if subcommand_cli.full_auto {
        interactive.full_auto = true;
    }
    if subcommand_cli.dangerously_bypass_approvals_and_sandbox {
        interactive.dangerously_bypass_approvals_and_sandbox = true;
    }
    if let Some(cwd) = subcommand_cli.cwd {
        interactive.cwd = Some(cwd);
    }
    if subcommand_cli.web_search {
        interactive.web_search = true;
    }
    if !subcommand_cli.images.is_empty() {
        interactive.images = subcommand_cli.images;
    }
    if !subcommand_cli.add_dir.is_empty() {
        interactive.add_dir.extend(subcommand_cli.add_dir);
    }
    if let Some(prompt) = subcommand_cli.prompt {
        // Normalize CRLF/CR to LF so CLI-provided text can't leak `\r` into TUI state.
        interactive.prompt = Some(prompt.replace("\r\n", "\n").replace('\r', "\n"));
    }

    interactive
        .config_overrides
        .raw_overrides
        .extend(subcommand_cli.config_overrides.raw_overrides);
}

fn print_completion(cmd: CompletionCommand) {
    let mut app = MultitoolCli::command();
    let name = "codex";
    generate(cmd.shell, &mut app, name, &mut std::io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::TokenUsage;
    use pretty_assertions::assert_eq;

    fn finalize_resume_from_args(args: &[&str]) -> TuiCli {
        let cli = MultitoolCli::try_parse_from(args).expect("parse");
        let MultitoolCli {
            interactive,
            config_overrides: root_overrides,
            subcommand,
            feature_toggles: _,
        } = cli;

        let Subcommand::Resume(ResumeCommand {
            session_id,
            last,
            all,
            config_overrides: resume_cli,
        }) = subcommand.expect("resume present")
        else {
            unreachable!()
        };

        finalize_resume_interactive(
            interactive,
            root_overrides,
            session_id,
            last,
            all,
            resume_cli,
        )
    }

    fn finalize_fork_from_args(args: &[&str]) -> TuiCli {
        let cli = MultitoolCli::try_parse_from(args).expect("parse");
        let MultitoolCli {
            interactive,
            config_overrides: root_overrides,
            subcommand,
            feature_toggles: _,
        } = cli;

        let Subcommand::Fork(ForkCommand {
            session_id,
            last,
            all,
            config_overrides: fork_cli,
        }) = subcommand.expect("fork present")
        else {
            unreachable!()
        };

        finalize_fork_interactive(interactive, root_overrides, session_id, last, all, fork_cli)
    }

    #[test]
    fn exec_resume_last_accepts_prompt_positional() {
        let cli =
            MultitoolCli::try_parse_from(["codex", "exec", "--json", "resume", "--last", "2+2"])
                .expect("parse should succeed");

        let Some(Subcommand::Exec(exec)) = cli.subcommand else {
            panic!("expected exec subcommand");
        };
        let Some(codex_exec::Command::Resume(args)) = exec.command else {
            panic!("expected exec resume");
        };

        assert!(args.last);
        assert_eq!(args.session_id, None);
        assert_eq!(args.prompt.as_deref(), Some("2+2"));
    }

    #[test]
    fn exec_resume_accepts_output_last_message_flag_after_subcommand() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "exec",
            "resume",
            "session-123",
            "-o",
            "/tmp/resume-output.md",
            "re-review",
        ])
        .expect("parse should succeed");

        let Some(Subcommand::Exec(exec)) = cli.subcommand else {
            panic!("expected exec subcommand");
        };
        let Some(codex_exec::Command::Resume(args)) = exec.command else {
            panic!("expected exec resume");
        };

        assert_eq!(
            exec.last_message_file,
            Some(std::path::PathBuf::from("/tmp/resume-output.md"))
        );
        assert_eq!(args.session_id.as_deref(), Some("session-123"));
        assert_eq!(args.prompt.as_deref(), Some("re-review"));
    }

    fn app_server_from_args(args: &[&str]) -> AppServerCommand {
        let cli = MultitoolCli::try_parse_from(args).expect("parse");
        let Subcommand::AppServer(app_server) = cli.subcommand.expect("app-server present") else {
            unreachable!()
        };
        app_server
    }

    fn sample_exit_info(conversation_id: Option<&str>, thread_name: Option<&str>) -> AppExitInfo {
        let token_usage = TokenUsage {
            output_tokens: 2,
            total_tokens: 2,
            ..Default::default()
        };
        AppExitInfo {
            token_usage,
            thread_id: conversation_id
                .map(ThreadId::from_string)
                .map(Result::unwrap),
            thread_name: thread_name.map(str::to_string),
            update_action: None,
            exit_reason: ExitReason::UserRequested,
        }
    }

    #[test]
    fn format_exit_messages_skips_zero_usage() {
        let exit_info = AppExitInfo {
            token_usage: TokenUsage::default(),
            thread_id: None,
            thread_name: None,
            update_action: None,
            exit_reason: ExitReason::UserRequested,
        };
        let lines = format_exit_messages(exit_info, false);
        assert!(lines.is_empty());
    }

    #[test]
    fn format_exit_messages_includes_resume_hint_without_color() {
        let exit_info = sample_exit_info(Some("123e4567-e89b-12d3-a456-426614174000"), None);
        let lines = format_exit_messages(exit_info, false);
        assert_eq!(
            lines,
            vec![
                "Token usage: total=2 input=0 output=2".to_string(),
                "To continue this session, run codex resume 123e4567-e89b-12d3-a456-426614174000"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn format_exit_messages_applies_color_when_enabled() {
        let exit_info = sample_exit_info(Some("123e4567-e89b-12d3-a456-426614174000"), None);
        let lines = format_exit_messages(exit_info, true);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("\u{1b}[36m"));
    }

    #[test]
    fn format_exit_messages_prefers_thread_name() {
        let exit_info = sample_exit_info(
            Some("123e4567-e89b-12d3-a456-426614174000"),
            Some("my-thread"),
        );
        let lines = format_exit_messages(exit_info, false);
        assert_eq!(
            lines,
            vec![
                "Token usage: total=2 input=0 output=2".to_string(),
                "To continue this session, run codex resume my-thread".to_string(),
            ]
        );
    }

    #[test]
    fn resume_model_flag_applies_when_no_root_flags() {
        let interactive =
            finalize_resume_from_args(["codex", "resume", "-m", "gpt-5.1-test"].as_ref());

        assert_eq!(interactive.model.as_deref(), Some("gpt-5.1-test"));
        assert!(interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
    }

    #[test]
    fn resume_picker_logic_none_and_not_last() {
        let interactive = finalize_resume_from_args(["codex", "resume"].as_ref());
        assert!(interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_picker_logic_last() {
        let interactive = finalize_resume_from_args(["codex", "resume", "--last"].as_ref());
        assert!(!interactive.resume_picker);
        assert!(interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_picker_logic_with_session_id() {
        let interactive = finalize_resume_from_args(["codex", "resume", "1234"].as_ref());
        assert!(!interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id.as_deref(), Some("1234"));
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_all_flag_sets_show_all() {
        let interactive = finalize_resume_from_args(["codex", "resume", "--all"].as_ref());
        assert!(interactive.resume_picker);
        assert!(interactive.resume_show_all);
    }

    #[test]
    fn resume_merges_option_flags_and_full_auto() {
        let interactive = finalize_resume_from_args(
            [
                "codex",
                "resume",
                "sid",
                "--oss",
                "--full-auto",
                "--search",
                "--sandbox",
                "workspace-write",
                "--ask-for-approval",
                "on-request",
                "-m",
                "gpt-5.1-test",
                "-p",
                "my-profile",
                "-C",
                "/tmp",
                "-i",
                "/tmp/a.png,/tmp/b.png",
            ]
            .as_ref(),
        );

        assert_eq!(interactive.model.as_deref(), Some("gpt-5.1-test"));
        assert!(interactive.oss);
        assert_eq!(interactive.config_profile.as_deref(), Some("my-profile"));
        assert_matches!(
            interactive.sandbox_mode,
            Some(codex_utils_cli::SandboxModeCliArg::WorkspaceWrite)
        );
        assert_matches!(
            interactive.approval_policy,
            Some(codex_utils_cli::ApprovalModeCliArg::OnRequest)
        );
        assert!(interactive.full_auto);
        assert_eq!(
            interactive.cwd.as_deref(),
            Some(std::path::Path::new("/tmp"))
        );
        assert!(interactive.web_search);
        let has_a = interactive
            .images
            .iter()
            .any(|p| p == std::path::Path::new("/tmp/a.png"));
        let has_b = interactive
            .images
            .iter()
            .any(|p| p == std::path::Path::new("/tmp/b.png"));
        assert!(has_a && has_b);
        assert!(!interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id.as_deref(), Some("sid"));
    }

    #[test]
    fn resume_merges_dangerously_bypass_flag() {
        let interactive = finalize_resume_from_args(
            [
                "codex",
                "resume",
                "--dangerously-bypass-approvals-and-sandbox",
            ]
            .as_ref(),
        );
        assert!(interactive.dangerously_bypass_approvals_and_sandbox);
        assert!(interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
    }

    #[test]
    fn fork_picker_logic_none_and_not_last() {
        let interactive = finalize_fork_from_args(["codex", "fork"].as_ref());
        assert!(interactive.fork_picker);
        assert!(!interactive.fork_last);
        assert_eq!(interactive.fork_session_id, None);
        assert!(!interactive.fork_show_all);
    }

    #[test]
    fn fork_picker_logic_last() {
        let interactive = finalize_fork_from_args(["codex", "fork", "--last"].as_ref());
        assert!(!interactive.fork_picker);
        assert!(interactive.fork_last);
        assert_eq!(interactive.fork_session_id, None);
        assert!(!interactive.fork_show_all);
    }

    #[test]
    fn fork_picker_logic_with_session_id() {
        let interactive = finalize_fork_from_args(["codex", "fork", "1234"].as_ref());
        assert!(!interactive.fork_picker);
        assert!(!interactive.fork_last);
        assert_eq!(interactive.fork_session_id.as_deref(), Some("1234"));
        assert!(!interactive.fork_show_all);
    }

    #[test]
    fn fork_all_flag_sets_show_all() {
        let interactive = finalize_fork_from_args(["codex", "fork", "--all"].as_ref());
        assert!(interactive.fork_picker);
        assert!(interactive.fork_show_all);
    }

    #[test]
    fn app_server_analytics_default_disabled_without_flag() {
        let app_server = app_server_from_args(["codex", "app-server"].as_ref());
        assert!(!app_server.analytics_default_enabled);
        assert_eq!(
            app_server.listen,
            codex_app_server::AppServerTransport::Stdio
        );
    }

    #[test]
    fn app_server_analytics_default_enabled_with_flag() {
        let app_server =
            app_server_from_args(["codex", "app-server", "--analytics-default-enabled"].as_ref());
        assert!(app_server.analytics_default_enabled);
    }

    #[test]
    fn app_server_listen_websocket_url_parses() {
        let app_server = app_server_from_args(
            ["codex", "app-server", "--listen", "ws://127.0.0.1:4500"].as_ref(),
        );
        assert_eq!(
            app_server.listen,
            codex_app_server::AppServerTransport::WebSocket {
                bind_address: "127.0.0.1:4500".parse().expect("valid socket address"),
            }
        );
    }

    #[test]
    fn app_server_listen_stdio_url_parses() {
        let app_server =
            app_server_from_args(["codex", "app-server", "--listen", "stdio://"].as_ref());
        assert_eq!(
            app_server.listen,
            codex_app_server::AppServerTransport::Stdio
        );
    }

    #[test]
    fn app_server_listen_invalid_url_fails_to_parse() {
        let parse_result =
            MultitoolCli::try_parse_from(["codex", "app-server", "--listen", "http://foo"]);
        assert!(parse_result.is_err());
    }

    #[test]
    fn features_enable_parses_feature_name() {
        let cli = MultitoolCli::try_parse_from(["codex", "features", "enable", "unified_exec"])
            .expect("parse should succeed");
        let Some(Subcommand::Features(FeaturesCli { sub })) = cli.subcommand else {
            panic!("expected features subcommand");
        };
        let FeaturesSubcommand::Enable(FeatureSetArgs { feature }) = sub else {
            panic!("expected features enable");
        };
        assert_eq!(feature, "unified_exec");
    }

    #[test]
    fn features_disable_parses_feature_name() {
        let cli = MultitoolCli::try_parse_from(["codex", "features", "disable", "shell_tool"])
            .expect("parse should succeed");
        let Some(Subcommand::Features(FeaturesCli { sub })) = cli.subcommand else {
            panic!("expected features subcommand");
        };
        let FeaturesSubcommand::Disable(FeatureSetArgs { feature }) = sub else {
            panic!("expected features disable");
        };
        assert_eq!(feature, "shell_tool");
    }

    #[test]
    fn process_run_parses_task() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "process",
            "run",
            "--task",
            "Implement process mode",
        ])
        .expect("parse should succeed");
        let Some(Subcommand::Process(ProcessCli { sub })) = cli.subcommand else {
            panic!("expected process subcommand");
        };
        let ProcessSubcommand::Run(ProcessRunArgs { task }) = sub else {
            panic!("expected process run");
        };
        assert_eq!(task, "Implement process mode");
    }

    #[test]
    fn process_pr_comments_parses_target() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "process",
            "pr-comments",
            "--repo",
            "njfio/codex-process",
            "--pr",
            "1",
        ])
        .expect("parse should succeed");
        let Some(Subcommand::Process(ProcessCli { sub })) = cli.subcommand else {
            panic!("expected process subcommand");
        };
        let ProcessSubcommand::PrComments(ProcessPrCommentsArgs { repo, pr, act }) = sub else {
            panic!("expected process pr-comments");
        };
        assert_eq!(repo, "njfio/codex-process");
        assert_eq!(pr, 1);
        assert!(!act);
    }

    #[test]
    fn process_pr_comments_parses_act_mode() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "process",
            "pr-comments",
            "--repo",
            "njfio/codex-process",
            "--pr",
            "1",
            "--act",
        ])
        .expect("parse should succeed");
        let Some(Subcommand::Process(ProcessCli { sub })) = cli.subcommand else {
            panic!("expected process subcommand");
        };
        let ProcessSubcommand::PrComments(ProcessPrCommentsArgs { act, .. }) = sub else {
            panic!("expected process pr-comments");
        };
        assert!(act);
    }

    #[test]
    fn process_issues_watch_parses_args() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "process",
            "issues",
            "watch",
            "--repo",
            "njfio/codex-process",
            "--label",
            "process:auto-fix",
            "--limit",
            "20",
        ])
        .expect("parse should succeed");
        let Some(Subcommand::Process(ProcessCli { sub })) = cli.subcommand else {
            panic!("expected process subcommand");
        };
        let ProcessSubcommand::Issues(ProcessIssuesCli { sub }) = sub else {
            panic!("expected process issues");
        };
        let ProcessIssuesSubcommand::Watch(ProcessIssuesWatchArgs {
            repo,
            label,
            limit,
            act,
        }) = sub;
        assert_eq!(repo, "njfio/codex-process");
        assert_eq!(label, "process:auto-fix");
        assert_eq!(limit, 20);
        assert!(!act);
    }

    #[test]
    fn process_issues_watch_parses_act_mode() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "process",
            "issues",
            "watch",
            "--repo",
            "njfio/codex-process",
            "--label",
            "process:auto-fix",
            "--limit",
            "20",
            "--act",
        ])
        .expect("parse should succeed");
        let Some(Subcommand::Process(ProcessCli { sub })) = cli.subcommand else {
            panic!("expected process subcommand");
        };
        let ProcessSubcommand::Issues(ProcessIssuesCli { sub }) = sub else {
            panic!("expected process issues");
        };
        let ProcessIssuesSubcommand::Watch(ProcessIssuesWatchArgs { act, .. }) = sub;
        assert!(act);
    }

    #[test]
    fn parse_repo_owner_and_name_accepts_valid_repo() {
        let parsed = parse_repo_owner_and_name("openai/codex").expect("repo parses");
        assert_eq!(parsed, ("openai".to_string(), "codex".to_string()));
    }

    #[test]
    fn parse_repo_owner_and_name_rejects_invalid_repo() {
        let err = parse_repo_owner_and_name("openai").expect_err("repo should fail");
        assert_eq!(
            err.to_string(),
            "Invalid --repo value `openai`. Expected `owner/name`."
        );
    }

    #[test]
    fn parse_repo_owner_and_name_rejects_too_many_path_segments() {
        let err = parse_repo_owner_and_name("openai/codex/extra").expect_err("repo should fail");
        assert_eq!(
            err.to_string(),
            "Invalid --repo value `openai/codex/extra`. Expected `owner/name`."
        );
    }

    #[test]
    fn parse_unresolved_review_comment_page_filters_out_resolved_threads() {
        let input = serde_json::json!({
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [
                                {
                                    "id": "PRRT_1",
                                    "isResolved": false,
                                    "comments": {
                                        "nodes": [
                                            {
                                                "id": "PRRC_1",
                                                "author": { "login": "alice" },
                                                "path": "src/lib.rs",
                                                "line": 42,
                                                "body": "Please simplify this branch.",
                                                "url": "https://github.com/openai/codex/pull/1#discussion_r1"
                                            }
                                        ]
                                    }
                                },
                                {
                                    "id": "PRRT_2",
                                    "isResolved": true,
                                    "comments": {
                                        "nodes": [
                                            {
                                                "id": "PRRC_2",
                                                "author": { "login": "bob" },
                                                "path": "src/main.rs",
                                                "line": 7,
                                                "body": "Already fixed.",
                                                "url": "https://github.com/openai/codex/pull/1#discussion_r2"
                                            }
                                        ]
                                    }
                                }
                            ],
                            "pageInfo": {
                                "hasNextPage": false,
                                "endCursor": null
                            }
                        }
                    }
                }
            }
        });

        let parsed = parse_unresolved_review_comment_page(input).expect("parses");
        assert_eq!(
            parsed.comments,
            vec![UnresolvedReviewComment {
                id: "PRRC_1".to_string(),
                review_thread_id: Some("PRRT_1".to_string()),
                author: "alice".to_string(),
                path: "src/lib.rs".to_string(),
                line: Some(42),
                body: "Please simplify this branch.".to_string(),
                url: "https://github.com/openai/codex/pull/1#discussion_r1".to_string(),
            }]
        );
        assert!(!parsed.has_next_page);
        assert_eq!(parsed.end_cursor, None);
    }

    #[test]
    fn parse_issue_comments_supports_paginated_response() {
        let input = serde_json::json!([
            [
                {
                    "id": 101,
                    "body": "Can you add a test?",
                    "html_url": "https://github.com/openai/codex/issues/1#issuecomment-101",
                    "user": { "login": "carol" }
                }
            ],
            [
                {
                    "id": 202,
                    "body": "Looks good after updates.",
                    "html_url": "https://github.com/openai/codex/issues/1#issuecomment-202",
                    "user": { "login": "dave" }
                }
            ]
        ]);

        let parsed = parse_issue_comments(input).expect("parses");
        assert_eq!(
            parsed,
            vec![
                OpenIssueComment {
                    id: "101".to_string(),
                    author: "carol".to_string(),
                    body: "Can you add a test?".to_string(),
                    url: "https://github.com/openai/codex/issues/1#issuecomment-101".to_string(),
                },
                OpenIssueComment {
                    id: "202".to_string(),
                    author: "dave".to_string(),
                    body: "Looks good after updates.".to_string(),
                    url: "https://github.com/openai/codex/issues/1#issuecomment-202".to_string(),
                }
            ]
        );
    }

    #[test]
    fn suggested_next_actions_handles_empty_and_non_empty_states() {
        let empty = GroupedByType {
            unresolved_review_comments: 0,
            open_issue_comments: 0,
            total: 0,
        };
        assert_eq!(
            suggested_next_actions(&empty),
            vec![
                "No open comments found; proceed with final verification and merge checks."
                    .to_string()
            ]
        );

        let non_empty = GroupedByType {
            unresolved_review_comments: 1,
            open_issue_comments: 2,
            total: 3,
        };
        assert_eq!(
            suggested_next_actions(&non_empty),
            vec![
                "Address unresolved review comments first.".to_string(),
                "Reply to open issue comments on the PR conversation.".to_string(),
                "After fixes, rerun `codex process pr-comments --repo <owner/name> --pr <number>` to confirm all comments are addressed.".to_string(),
            ]
        );
    }

    #[test]
    fn classify_comment_triage_prioritizes_needs_issue_keywords() {
        let decision =
            classify_comment_triage("This is out of scope for this PR, please open a follow-up.");
        assert_eq!(decision, TriageDecision::NeedsIssue);
    }

    #[test]
    fn classify_comment_triage_detects_question() {
        let decision = classify_comment_triage("Can we simplify this flow?");
        assert_eq!(decision, TriageDecision::Question);
    }

    #[test]
    fn classify_comment_triage_defaults_to_quick_fix() {
        let decision = classify_comment_triage("Please update this implementation detail.");
        assert_eq!(decision, TriageDecision::QuickFix);
    }

    #[test]
    fn classify_issue_watch_triage_detects_small_change_keywords() {
        let decision = classify_issue_watch_triage(&ProcessWatchIssueCandidate {
            number: 10,
            title: "Fix typo in docs".to_string(),
            body: String::new(),
            url: "https://github.com/openai/codex-process/issues/10".to_string(),
        });
        assert_eq!(decision, IssueWatchDecision::QuickFix);
    }

    #[test]
    fn classify_issue_watch_triage_defaults_to_needs_manual() {
        let decision = classify_issue_watch_triage(&ProcessWatchIssueCandidate {
            number: 11,
            title: "Refactor process engine".to_string(),
            body: "This likely spans multiple crates.".to_string(),
            url: "https://github.com/openai/codex-process/issues/11".to_string(),
        });
        assert_eq!(decision, IssueWatchDecision::NeedsManual);
    }

    #[test]
    fn parse_quick_fix_output_extracts_summary_files_and_verification() {
        let output = "SUMMARY: Rename local variable for clarity\nFILES: cli/src/main.rs, README.md\nVERIFICATION: not run";
        let parsed = parse_quick_fix_output(output);
        assert_eq!(
            parsed,
            ParsedQuickFixOutput {
                summary: Some("Rename local variable for clarity".to_string()),
                files: vec!["cli/src/main.rs".to_string(), "README.md".to_string()],
                verification: Some("not run".to_string()),
            }
        );
    }

    #[test]
    fn format_follow_up_issue_body_renders_readable_markdown() {
        let body = format_follow_up_issue_body(
            "openai/codex",
            42,
            "https://github.com/openai/codex/pull/42#discussion_r1",
            "Please fix this.\n\nNeed tests.",
        );
        assert_eq!(
            body,
            "Created by `codex process pr-comments --act`.\n\n- Source PR: https://github.com/openai/codex/pull/42\n- Source comment: https://github.com/openai/codex/pull/42#discussion_r1\n\nOriginal comment:\n\n> Please fix this.\n>\n> Need tests.\n"
                .to_string()
        );
    }

    #[test]
    fn extract_first_url_returns_http_url_with_trailing_punctuation_trimmed() {
        let text = "comment posted: https://github.com/openai/codex/pull/1#issuecomment-2). done";
        let parsed = extract_first_url(text);
        assert_eq!(
            parsed,
            Some("https://github.com/openai/codex/pull/1#issuecomment-2".to_string())
        );
    }

    #[test]
    fn format_pr_update_comment_body_includes_files_and_verification_when_available() {
        let body = format_pr_update_comment_body(&[QuickFixSummary {
            comment_id: "PRRC_123".to_string(),
            summary: "Applied minimal fix".to_string(),
            files: vec!["codex-rs/cli/src/main.rs".to_string()],
            verification: Some("not run".to_string()),
            commit_sha: Some("0123456789abcdef".to_string()),
            commit_url: Some(
                "https://github.com/openai/codex-process/commit/0123456789abcdef".to_string(),
            ),
            follow_up_pr_url: Some("https://github.com/openai/codex-process/pull/77".to_string()),
            follow_up_pr_number: Some(77),
            thread_resolved: Some(true),
            thread_resolve_error: None,
        }]);
        assert_eq!(
            body,
            "Quick-fix update from `codex process pr-comments --act`.\n\nApplied items:\n- `PRRC_123`: Applied minimal fix (files: codex-rs/cli/src/main.rs); verification: not run; commit: [`0123456789ab`](https://github.com/openai/codex-process/commit/0123456789abcdef); follow-up PR: [#77](https://github.com/openai/codex-process/pull/77); review thread: resolved\n"
                .to_string()
        );
    }

    #[test]
    fn format_pr_update_comment_body_includes_thread_resolution_failure_details() {
        let body = format_pr_update_comment_body(&[QuickFixSummary {
            comment_id: "PRRC_321".to_string(),
            summary: "Applied follow-up change".to_string(),
            files: Vec::new(),
            verification: None,
            commit_sha: None,
            commit_url: None,
            follow_up_pr_url: Some("https://github.com/openai/codex-process/pull/89".to_string()),
            follow_up_pr_number: Some(89),
            thread_resolved: Some(false),
            thread_resolve_error: Some("Forbidden".to_string()),
        }]);
        assert_eq!(
            body,
            "Quick-fix update from `codex process pr-comments --act`.\n\nApplied items:\n- `PRRC_321`: Applied follow-up change; follow-up PR: [#89](https://github.com/openai/codex-process/pull/89); review thread: resolution failed (Forbidden)\n"
                .to_string()
        );
    }

    #[test]
    fn parse_gh_pr_create_output_extracts_url_and_number() {
        let parsed = parse_gh_pr_create_output(
            "https://github.com/openai/codex-process/pull/88\nCreated pull request #88",
        );
        assert_eq!(
            parsed,
            ParsedGhPrCreateOutput {
                url: Some("https://github.com/openai/codex-process/pull/88".to_string()),
                number: Some(88),
            }
        );
    }

    #[test]
    fn parse_gh_pr_create_output_extracts_number_when_only_hash_is_present() {
        let parsed = parse_gh_pr_create_output("Created pull request #91");
        assert_eq!(
            parsed,
            ParsedGhPrCreateOutput {
                url: None,
                number: Some(91),
            }
        );
    }

    #[test]
    fn parse_gh_pr_create_output_prefers_pull_url_over_other_urls() {
        let parsed = parse_gh_pr_create_output(
            "Compare at https://github.com/openai/codex-process/compare/a...b and PR https://github.com/openai/codex-process/pull/99",
        );
        assert_eq!(
            parsed,
            ParsedGhPrCreateOutput {
                url: Some("https://github.com/openai/codex-process/pull/99".to_string()),
                number: Some(99),
            }
        );
    }

    #[test]
    fn parse_gh_issue_comment_output_extracts_issue_comment_url() {
        let parsed = parse_gh_issue_comment_output(
            "https://github.com/openai/codex-process/issues/42#issuecomment-9999",
        );
        assert_eq!(
            parsed,
            ParsedGhIssueCommentOutput {
                url: Some(
                    "https://github.com/openai/codex-process/issues/42#issuecomment-9999"
                        .to_string()
                ),
            }
        );
    }

    #[test]
    fn parse_gh_issue_comment_output_ignores_non_issue_urls() {
        let parsed = parse_gh_issue_comment_output(
            "comment posted: https://github.com/openai/codex-process/pull/42#issuecomment-1000",
        );
        assert_eq!(parsed, ParsedGhIssueCommentOutput { url: None });
    }

    #[test]
    fn parse_gh_issue_comment_output_prefers_issue_comment_url_when_multiple_urls_present() {
        let parsed = parse_gh_issue_comment_output(
            "Compare: https://github.com/openai/codex-process/compare/main...branch Comment: https://github.com/openai/codex-process/issues/42#issuecomment-123",
        );
        assert_eq!(
            parsed,
            ParsedGhIssueCommentOutput {
                url: Some(
                    "https://github.com/openai/codex-process/issues/42#issuecomment-123"
                        .to_string()
                ),
            }
        );
    }

    #[test]
    fn parse_resolve_review_thread_response_accepts_resolved_thread() {
        let input = serde_json::json!({
            "data": {
                "resolveReviewThread": {
                    "thread": {
                        "id": "PRRT_1",
                        "isResolved": true
                    }
                }
            }
        });
        parse_resolve_review_thread_response(input).expect("parse should succeed");
    }

    #[test]
    fn parse_resolve_review_thread_response_reports_graphql_errors() {
        let input = serde_json::json!({
            "errors": [
                { "message": "Forbidden" }
            ]
        });
        let err = parse_resolve_review_thread_response(input).expect_err("parse should fail");
        assert_eq!(err.to_string(), "GitHub GraphQL returned errors: Forbidden");
    }

    #[test]
    fn parse_resolve_review_thread_response_reports_missing_thread_data() {
        let input = serde_json::json!({
            "data": {
                "resolveReviewThread": {
                    "thread": null
                }
            }
        });
        let err = parse_resolve_review_thread_response(input).expect_err("parse should fail");
        assert_eq!(
            err.to_string(),
            "GitHub GraphQL response did not include resolveReviewThread.thread data."
        );
    }

    #[test]
    fn format_quick_fix_follow_up_pr_body_includes_required_links_and_optional_commit() {
        let body = format_quick_fix_follow_up_pr_body(
            "openai/codex-process",
            42,
            &ProcessCommentTriageItem {
                source: "review_comment".to_string(),
                comment_id: "PRRC_1".to_string(),
                review_thread_id: Some("PRRT_1".to_string()),
                author: "reviewer".to_string(),
                body: "Please tighten this check.".to_string(),
                comment_url: "https://github.com/openai/codex-process/pull/42#discussion_r1"
                    .to_string(),
                decision: TriageDecision::QuickFix,
                created_issue_url: None,
                todo: None,
                quick_fix_attempted: true,
                quick_fix_success: Some(true),
                quick_fix_summary: None,
                quick_fix_error: None,
                quick_fix_branch: Some("process/quick-fix-pr-42-prrc-1".to_string()),
                quick_fix_commit_sha: Some("abc".to_string()),
                quick_fix_commit_url: Some(
                    "https://github.com/openai/codex-process/commit/abc".to_string(),
                ),
                quick_fix_pushed: Some(true),
                quick_fix_remote_branch: Some("process/quick-fix-pr-42-prrc-1".to_string()),
                quick_fix_pr_url: None,
                quick_fix_pr_number: None,
                quick_fix_push_error: None,
                quick_fix_pr_error: None,
                quick_fix_thread_resolved: None,
                quick_fix_thread_resolve_error: None,
            },
            Some("https://github.com/openai/codex-process/commit/abc"),
        );
        assert_eq!(
            body,
            "Created by `codex process pr-comments --act`.\n\n- Source PR: https://github.com/openai/codex-process/pull/42\n- Source comment: https://github.com/openai/codex-process/pull/42#discussion_r1\n- Quick-fix commit: https://github.com/openai/codex-process/commit/abc\n\nOriginal comment:\n\n> Please tighten this check.\n"
                .to_string()
        );
    }

    #[test]
    fn format_issue_watch_follow_up_pr_body_includes_issue_and_commit_context() {
        let body = format_issue_watch_follow_up_pr_body(
            "openai/codex-process",
            &ProcessWatchIssueCandidate {
                number: 123,
                title: "Fix docs typo in process mode".to_string(),
                body: "Typo in README process section.".to_string(),
                url: "https://github.com/openai/codex-process/issues/123".to_string(),
            },
            Some("https://github.com/openai/codex-process/commit/abc123"),
        );
        assert_eq!(
            body,
            "Created by `codex process issues watch --act`.\n\n- Source issue: https://github.com/openai/codex-process/issues/123\n- Repository: https://github.com/openai/codex-process\n- Quick-fix commit: https://github.com/openai/codex-process/commit/abc123\n\nIssue summary:\n\n- #123: Fix docs typo in process mode\n"
                .to_string()
        );
    }

    #[test]
    fn format_issue_watch_manual_follow_up_comment_is_concise() {
        let body = format_issue_watch_manual_follow_up_comment("requires architectural changes");
        assert_eq!(
            body,
            "Automation update from `codex process issues watch --act`.\n\nManual follow-up needed: requires architectural changes\n"
                .to_string()
        );
    }

    #[test]
    fn format_issue_watch_success_comment_includes_pr_and_commit_links() {
        let body = format_issue_watch_success_comment(
            Some("Updated docs and fixed typo"),
            Some("https://github.com/openai/codex-process/pull/200"),
            Some(200),
            Some("https://github.com/openai/codex-process/commit/abc123"),
        );
        assert_eq!(
            body,
            "Automation update from `codex process issues watch --act`.\n\n- Result: Updated docs and fixed typo\n- Follow-up PR: [#200](https://github.com/openai/codex-process/pull/200)\n- Commit: https://github.com/openai/codex-process/commit/abc123\n"
                .to_string()
        );
    }

    #[test]
    fn quick_fix_branch_name_is_deterministic_and_safe() {
        let branch = quick_fix_branch_name(123, "PRRC_ABC/123");
        assert_eq!(branch, "process/quick-fix-pr-123-prrc-abc-123".to_string());
    }

    #[test]
    fn quick_fix_commit_url_uses_repo_and_sha() {
        let url = quick_fix_commit_url("openai/codex-process", "abc123");
        assert_eq!(
            url,
            "https://github.com/openai/codex-process/commit/abc123".to_string()
        );
    }

    #[test]
    fn short_commit_sha_truncates_long_values() {
        assert_eq!(short_commit_sha("0123456789abcdef"), "0123456789ab");
        assert_eq!(short_commit_sha("abc123"), "abc123");
    }

    #[test]
    fn feature_toggles_known_features_generate_overrides() {
        let toggles = FeatureToggles {
            enable: vec!["web_search_request".to_string()],
            disable: vec!["unified_exec".to_string()],
        };
        let overrides = toggles.to_overrides().expect("valid features");
        assert_eq!(
            overrides,
            vec![
                "features.web_search_request=true".to_string(),
                "features.unified_exec=false".to_string(),
            ]
        );
    }

    #[test]
    fn feature_toggles_unknown_feature_errors() {
        let toggles = FeatureToggles {
            enable: vec!["does_not_exist".to_string()],
            disable: Vec::new(),
        };
        let err = toggles
            .to_overrides()
            .expect_err("feature should be rejected");
        assert_eq!(err.to_string(), "Unknown feature flag: does_not_exist");
    }
}
