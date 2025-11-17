use atomic_lib::agents::Agent;
use atomic_lib::config::Config;
use atomic_lib::config::{ClientConfig, SharedConfig};
use atomic_lib::mapping::Mapping;
use atomic_lib::serialize::Format;
use atomic_lib::{errors::AtomicResult, Storelike};
use clap::{crate_version, Parser, Subcommand, ValueEnum};
use colored::*;
use dirs::home_dir;
use std::{cell::RefCell, path::PathBuf, sync::Mutex};

mod commit;
mod get;
mod new;
mod print;
mod search;
mod validate;

#[derive(Parser)]
#[command(
    name = "atomic-cli",
    version = crate_version!(),
    author = "Joep Meindertsma <joep@ontola.io>",
    about = "Create, share, fetch and model Atomic Data!",
    after_help = "Visit https://atomicdata.dev for more info",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Create a Resource
    New {
        /// The URL or shortname of the Class that should be created
        #[arg(required = true)]
        class: String,
    },
    /// Get a Resource or Value by using Atomic Paths
    #[command(after_help = "\
        Traverses a Path and prints the resulting Resource or Value. \n\n\
        Examples: \n\n\
        $ atomic get class https://atomicdata.dev/properties/description\n\
        $ atomic get class description\n\
        $ atomic get https://example.com \n\n\
        Visit https://docs.atomicdata.dev/core/paths.html for more info about paths. \
    ")]
    Get {
        /// The subject URL
        #[arg(required = true)]
        subject: String,

        /// Serialization format
        #[arg(long, value_enum, default_value = "pretty")]
        as_: SerializeOptions,
    },
    /// Update a single Atom. Creates both the Resource if they don't exist. Overwrites existing.
    Set {
        /// Subject URL or bookmark of the resource
        #[arg(required = true)]
        subject: String,

        /// Property URL or shortname of the property
        #[arg(required = true)]
        property: String,

        /// String representation of the Value to be changed
        #[arg(required = true)]
        value: String,
    },
    /// Remove a single Atom from a Resource.
    Remove {
        /// Subject URL or bookmark of the resource
        #[arg(required = true)]
        subject: String,

        /// Property URL or shortname of the property to be deleted
        #[arg(required = true)]
        property: String,
    },
    /// Edit a single Atom from a Resource using your text editor.
    Edit {
        /// Subject URL or bookmark of the resource
        #[arg(required = true)]
        subject: String,

        /// Property URL or shortname of the property to be edited
        #[arg(required = true)]
        property: String,
    },
    /// Permanently removes a Resource.
    Destroy {
        /// Subject URL or bookmark of the resource to be destroyed
        #[arg(required = true)]
        subject: String,
    },
    /// Full text search
    Search {
        /// The search query
        #[arg(required = true)]
        query: String,
        /// Subject URL of the parent Resource to filter by
        #[arg(long)]
        parent: Option<String>,
        /// Server URL to search on
        /// Will query this + `/search` if provided.
        /// Defaults to the server in the config.
        #[arg(long)]
        server: Option<String>,
        /// Serialization format
        #[arg(long, value_enum, default_value = "pretty")]
        as_: SerializeOptions,
    },
    /// List all bookmarks
    List,
    /// Validates the store
    #[command(hide = true)]
    Validate,
    /// Print the current agent
    Agent,
    /// Validate an Atomic Server instance (full validation service)
    ValidateServer {
        /// Server URL to validate
        #[arg(required = true)]
        url: String,

        /// Agent secret for authentication (optional)
        #[arg(long)]
        agent: Option<String>,

        /// Validation level (0=Structural, 1=Datatype, 2=Schema, 3=Referential, 4=Cryptographic, 5=Authorization)
        #[arg(long, default_value = "2")]
        level: u8,

        /// Output format (json or text)
        #[arg(long, default_value = "text")]
        output: String,
    },
    /// Extract an ontology from a server to JSON-AD file
    ExtractOntology {
        /// Ontology URL to extract
        #[arg(required = true)]
        url: String,

        /// Output file path
        #[arg(long, required = true)]
        out: String,

        /// Agent secret for authentication
        #[arg(long)]
        agent: Option<String>,
    },
    /// Detect schema version of a server (V1=10 datatypes, V2=12 datatypes with Uri/JSON)
    DetectVersion {
        /// Server URL to check
        #[arg(required = true)]
        url: String,

        /// Agent secret for authentication
        #[arg(long)]
        agent: Option<String>,
    },
    /// Generate a diff report between two Atomic Server instances
    DiffServers {
        /// Source server URL
        #[arg(required = true)]
        source: String,

        /// Target server URL
        #[arg(required = true)]
        target: String,

        /// Agent secret for authentication
        #[arg(long)]
        agent: Option<String>,

        /// Output format (json or text)
        #[arg(long, default_value = "text")]
        output: String,
    },
    /// Synchronize data between two Atomic Server instances
    SyncServers {
        /// Source server URL
        #[arg(required = true)]
        source: String,

        /// Target server URL
        #[arg(required = true)]
        target: String,

        /// Agent secret for authentication (required for write operations)
        #[arg(long)]
        agent: Option<String>,

        /// Sync mode: push, pull, or bidirectional
        #[arg(long, default_value = "push")]
        mode: String,

        /// Conflict resolution strategy: source, target, latest, manual, or skip
        #[arg(long, default_value = "source")]
        conflict_strategy: String,

        /// Perform a dry run without making changes
        #[arg(long)]
        dry_run: bool,

        /// Include ontologies in sync
        #[arg(long)]
        include_ontologies: bool,

        /// Filter to only sync specific subject patterns (comma-separated)
        #[arg(long)]
        filter_subjects: Option<String>,

        /// Output format (json or text)
        #[arg(long, default_value = "text")]
        output: String,
    },
    /// Test connectivity to an Atomic Server
    TestConnection {
        /// Server URL to test
        #[arg(required = true)]
        url: String,

        /// Agent secret for authentication
        #[arg(long)]
        agent: Option<String>,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum SerializeOptions {
    Pretty,
    Json,
    JsonAd,
    NTriples,
}

impl Into<Format> for SerializeOptions {
    fn into(self) -> Format {
        match self {
            SerializeOptions::Pretty => Format::Pretty,
            SerializeOptions::Json => Format::Json,
            SerializeOptions::JsonAd => Format::JsonAd,
            SerializeOptions::NTriples => Format::NTriples,
        }
    }
}

#[allow(dead_code)]
/// The Context contains all the data for executing a single CLI command, such as the passed arguments and the in memory store.
pub struct Context {
    store: atomic_lib::Store,
    mapping: Mutex<Mapping>,
    matches: Commands,
    config_folder: PathBuf,
    user_mapping_path: PathBuf,
    /// A set of configuration options that are required for writing data on some server
    write: RefCell<Option<Config>>,
}

impl Context {
    /// Returns the config (agent, key) from the user config dir
    pub fn read_config(&self) -> Config {
        if let Some(write_ctx) = self.write.borrow().as_ref() {
            return write_ctx.clone();
        };
        let write_ctx =
            set_agent_config().expect("Issue while generating write context / agent configuration");
        self.write.borrow_mut().replace(write_ctx.clone());
        let agent = Agent::from_secret(&write_ctx.shared.agent_secret).unwrap();
        self.store.set_default_agent(agent);
        self.store
            .set_server_url(&write_ctx.client.clone().unwrap().server_url);

        write_ctx
    }
}

/// Reads config files for writing data, or promps the user if they don't yet exist
fn set_agent_config() -> CLIResult<Config> {
    let agent_config_path = atomic_lib::config::default_config_file_path()?;
    match atomic_lib::config::read_config(Some(&agent_config_path)) {
        Ok(found) => {
            prompt_for_missing_config_values(&found)?;
            Ok(found)
        }
        Err(_e) => {
            println!(
                "No config found at {:?}. Let's create one!",
                &agent_config_path
            );
            let server = promptly::prompt("What's the base url of your Atomic Server?")?;
            let agent_secret = promptly::prompt("Enter your agent secret")?;
            let config = atomic_lib::config::Config {
                shared: SharedConfig { agent_secret },
                client: Some(ClientConfig { server_url: server }),
            };
            config.save(&agent_config_path)?;
            println!("New config file created at {:?}", agent_config_path);
            Ok(config)
        }
    }
}

fn prompt_for_missing_config_values(config: &Config) -> AtomicResult<Config> {
    if config.client.is_none() {
        println!("No server url found in config.");
        let server = promptly::prompt("What's the base url of your Atomic Server?")
            .map_err(|e| format!("Invalid input: {}", e))?;
        let config = Config {
            client: Some(ClientConfig { server_url: server }),
            ..config.clone()
        };
        config.save(&atomic_lib::config::default_config_file_path()?)?;

        return Ok(config);
    }

    Ok(config.clone())
}

fn main() -> AtomicResult<()> {
    let cli = Cli::parse();

    let config_folder = home_dir()
        .expect("Home dir could not be opened. We need this to store some configuration files.")
        .join(".config/atomic/");

    // The mapping holds shortnames and URLs for quick CLI usage
    let mut mapping: Mapping = Mapping::init();
    let user_mapping_path = config_folder.join("mapping.amp");
    if !user_mapping_path.exists() {
        mapping.populate()?;
    } else {
        mapping.read_mapping_from_file(&user_mapping_path)?;
    }

    // Initialize an in-memory store
    let store = atomic_lib::Store::init()?;
    // Add some default data / common properties to speed things up
    store.populate()?;

    let mut context = Context {
        mapping: Mutex::new(mapping),
        store,
        matches: cli.command,
        config_folder,
        user_mapping_path,
        write: RefCell::new(None),
    };

    match exec_command(&mut context) {
        Ok(r) => r,
        Err(e) => {
            eprint!("{}", e);
            std::process::exit(1);
        }
    };

    Ok(())
}

fn exec_command(context: &mut Context) -> AtomicResult<()> {
    let command = context.matches.clone();

    match command {
        Commands::Destroy { subject } => {
            commit::destroy(context, &subject)?;
        }
        Commands::Edit { subject, property } => {
            #[cfg(feature = "native")]
            {
                commit::edit(context, &subject, &property)?;
            }
            #[cfg(not(feature = "native"))]
            {
                return Err("Feature not available. Compile with `native` feature.".into());
            }
        }
        Commands::Get { subject, as_ } => {
            get::get_resource(context, &subject, &as_)?;
        }
        Commands::List => {
            list(context);
        }
        Commands::New { class } => {
            new::new(context, &class)?;
        }
        Commands::Remove { subject, property } => {
            commit::remove(context, &subject, &property)?;
        }
        Commands::Set {
            subject,
            property,
            value,
        } => {
            commit::set(context, &subject, &property, &value)?;
        }
        Commands::Search {
            query,
            parent,
            server,
            as_,
        } => {
            search::search(context, query, parent, server, &as_)?;
        }
        Commands::Validate => {
            validate(context);
        }
        Commands::Agent => {
            let config = context.read_config();
            let agent = Agent::from_secret(&config.shared.agent_secret).unwrap();
            println!("{}", agent.subject);
        }
        Commands::ValidateServer {
            url,
            agent,
            level,
            output,
        } => {
            validate_server_command(&url, agent, level, &output)?;
        }
        Commands::ExtractOntology { url, out, agent } => {
            extract_ontology_command(&url, &out, agent)?;
        }
        Commands::DetectVersion { url, agent } => {
            detect_version_command(&url, agent)?;
        }
        Commands::DiffServers {
            source,
            target,
            agent,
            output,
        } => {
            diff_servers_command(&source, &target, agent, &output)?;
        }
        Commands::SyncServers {
            source,
            target,
            agent,
            mode,
            conflict_strategy,
            dry_run,
            include_ontologies,
            filter_subjects,
            output,
        } => {
            sync_servers_command(
                &source,
                &target,
                agent,
                &mode,
                &conflict_strategy,
                dry_run,
                include_ontologies,
                filter_subjects,
                &output,
            )?;
        }
        Commands::TestConnection { url, agent } => {
            test_connection_command(&url, agent)?;
        }
    };
    Ok(())
}

/// Validate an Atomic Server instance
fn validate_server_command(
    url: &str,
    agent_secret: Option<String>,
    level: u8,
    output_format: &str,
) -> AtomicResult<()> {
    println!(
        "{}",
        format!("Validating server: {}", url).blue().bold()
    );
    println!(
        "{}",
        format!("Validation level: {}", validate::ValidationLevel::from(level))
    );

    let validation_level = validate::ValidationLevel::from(level);

    match validate::validate_server(url, agent_secret, validation_level) {
        Ok(report) => {
            if output_format == "json" {
                let json = serde_json::to_string_pretty(&report)
                    .map_err(|e| format!("Failed to serialize report: {}", e))?;
                println!("{}", json);
            } else {
                // Text output
                println!("\n{}", "Validation Report".bold().underline());
                println!(
                    "Status: {}",
                    if report.valid {
                        "PASS".green().bold()
                    } else {
                        "FAIL".red().bold()
                    }
                );

                println!("\n{}", "Summary:".bold());
                println!("  Total Resources: {}", report.summary.total_resources);
                println!(
                    "  Valid: {}",
                    report.summary.valid_resources.to_string().green()
                );
                println!(
                    "  Invalid: {}",
                    if report.summary.invalid_resources > 0 {
                        report.summary.invalid_resources.to_string().red()
                    } else {
                        report.summary.invalid_resources.to_string().normal()
                    }
                );
                println!(
                    "  Schema Violations: {}",
                    report.summary.schema_violations
                );
                println!(
                    "  Missing References: {}",
                    report.summary.missing_references
                );

                if !report.errors.is_empty() {
                    println!(
                        "\n{}",
                        format!("Errors ({}):", report.errors.len()).red().bold()
                    );
                    for (i, error) in report.errors.iter().take(20).enumerate() {
                        println!(
                            "  {}. [{}] {}",
                            i + 1,
                            error.code.to_string().yellow(),
                            error.message
                        );
                        println!("     Subject: {}", error.subject.dimmed());
                        if let Some(prop) = &error.property {
                            println!("     Property: {}", prop.dimmed());
                        }
                    }
                    if report.errors.len() > 20 {
                        println!("  ... and {} more errors", report.errors.len() - 20);
                    }
                }

                if !report.warnings.is_empty() {
                    println!(
                        "\n{}",
                        format!("Warnings ({}):", report.warnings.len())
                            .yellow()
                            .bold()
                    );
                    for (i, warning) in report.warnings.iter().take(10).enumerate() {
                        println!("  {}. [{}] {}", i + 1, warning.code, warning.message);
                    }
                    if report.warnings.len() > 10 {
                        println!("  ... and {} more warnings", report.warnings.len() - 10);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Validation failed: {}", e).red());
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Extract an ontology to a JSON-AD file
fn extract_ontology_command(
    ontology_url: &str,
    output_path: &str,
    agent_secret: Option<String>,
) -> AtomicResult<()> {
    println!(
        "{}",
        format!("Extracting ontology: {}", ontology_url).blue().bold()
    );

    match validate::extract_ontology(ontology_url, agent_secret) {
        Ok(extracted_resources) => {
            // Collect all resources (main + referenced)
            let mut all_resources = Vec::new();
            for extracted in &extracted_resources {
                all_resources.push(extracted.main.clone());
                all_resources.extend(extracted.referenced.clone());
            }

            println!("  Found {} resources", all_resources.len());

            // Serialize to JSON-AD
            let json_ad = atomic_lib::Resource::vec_to_json_ad(&all_resources)
                .map_err(|e| format!("Failed to serialize to JSON-AD: {}", e))?;

            // Write to file
            std::fs::write(output_path, json_ad)
                .map_err(|e| format!("Failed to write to {}: {}", output_path, e))?;

            println!(
                "{}",
                format!("Successfully saved to: {}", output_path).green()
            );
            println!("  Total resources: {}", extracted_resources.len());
            println!(
                "  Including referenced: {}",
                all_resources.len() - extracted_resources.len()
            );
        }
        Err(e) => {
            eprintln!("{}", format!("Extraction failed: {}", e).red());
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Detect the schema version of a server
fn detect_version_command(server_url: &str, agent_secret: Option<String>) -> AtomicResult<()> {
    println!(
        "{}",
        format!("Detecting schema version for: {}", server_url)
            .blue()
            .bold()
    );

    match validate::detect_server_version(server_url, agent_secret) {
        Ok(version) => {
            let version_str = match version {
                validate::SchemaVersion::V1 => {
                    "V1 (10 datatypes - no Uri/JSON)".yellow().to_string()
                }
                validate::SchemaVersion::V2 => "V2 (12 datatypes - includes Uri/JSON)"
                    .green()
                    .to_string(),
            };
            println!("  Schema Version: {}", version_str);

            match version {
                validate::SchemaVersion::V1 => {
                    println!("  {} This server does not support Uri and JSON datatypes.", "WARNING:".yellow().bold());
                    println!("  Consider updating to the latest atomic-server.");
                }
                validate::SchemaVersion::V2 => {
                    println!("  {} Server supports all current Atomic Data features.", "OK:".green().bold());
                }
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Version detection failed: {}", e).red());
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Generate diff between two servers
fn diff_servers_command(
    source_url: &str,
    target_url: &str,
    agent_secret: Option<String>,
    output_format: &str,
) -> AtomicResult<()> {
    println!(
        "{}",
        format!("Comparing servers:\n  Source: {}\n  Target: {}", source_url, target_url)
            .blue()
            .bold()
    );

    match validate::diff_servers(source_url, target_url, agent_secret) {
        Ok(diff) => {
            if output_format == "json" {
                let json = serde_json::to_string_pretty(&diff)
                    .map_err(|e| format!("Failed to serialize diff: {}", e))?;
                println!("{}", json);
            } else {
                println!("\n{}", "Diff Report".bold().underline());
                println!("Source: {}", diff.source_url);
                println!("Target: {}", diff.target_url);

                println!("\n{}", "Summary:".bold());
                println!("  Total Differences: {}", diff.summary.total_differences);
                println!(
                    "  Resources Added (in source only): {}",
                    diff.summary.resources_added.to_string().green()
                );
                println!(
                    "  Resources Removed (in target only): {}",
                    diff.summary.resources_removed.to_string().red()
                );
                println!(
                    "  Resources Modified: {}",
                    diff.summary.resources_modified.to_string().yellow()
                );
                println!("  Schema Changes: {}", diff.summary.schema_changes);

                if !diff.source_only.is_empty() {
                    println!(
                        "\n{}",
                        format!("New Resources in Source ({}):", diff.source_only.len())
                            .green()
                            .bold()
                    );
                    for (i, subject) in diff.source_only.iter().take(10).enumerate() {
                        println!("  {}. {}", i + 1, subject);
                    }
                    if diff.source_only.len() > 10 {
                        println!("  ... and {} more", diff.source_only.len() - 10);
                    }
                }

                if !diff.target_only.is_empty() {
                    println!(
                        "\n{}",
                        format!("Resources Only in Target ({}):", diff.target_only.len())
                            .red()
                            .bold()
                    );
                    for (i, subject) in diff.target_only.iter().take(10).enumerate() {
                        println!("  {}. {}", i + 1, subject);
                    }
                    if diff.target_only.len() > 10 {
                        println!("  ... and {} more", diff.target_only.len() - 10);
                    }
                }

                if !diff.modified.is_empty() {
                    println!(
                        "\n{}",
                        format!("Modified Resources ({}):", diff.modified.len())
                            .yellow()
                            .bold()
                    );
                    for (i, resource_diff) in diff.modified.iter().take(10).enumerate() {
                        println!("  {}. {}", i + 1, resource_diff.subject);
                        println!(
                            "     Added props: {}, Removed: {}, Modified: {}",
                            resource_diff.added_properties.len(),
                            resource_diff.removed_properties.len(),
                            resource_diff.modified_properties.len()
                        );
                    }
                    if diff.modified.len() > 10 {
                        println!("  ... and {} more", diff.modified.len() - 10);
                    }
                }

                if diff.summary.schema_changes > 0 {
                    println!("\n{}", "Schema Changes:".bold());
                    if !diff.schema_changes.new_classes.is_empty() {
                        println!(
                            "  New Classes: {}",
                            diff.schema_changes.new_classes.len()
                        );
                    }
                    if !diff.schema_changes.new_properties.is_empty() {
                        println!(
                            "  New Properties: {}",
                            diff.schema_changes.new_properties.len()
                        );
                    }
                    if !diff.schema_changes.new_ontologies.is_empty() {
                        println!(
                            "  New Ontologies: {}",
                            diff.schema_changes.new_ontologies.len()
                        );
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Diff failed: {}", e).red());
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Synchronize data between two servers
fn sync_servers_command(
    source_url: &str,
    target_url: &str,
    agent_secret: Option<String>,
    mode: &str,
    conflict_strategy: &str,
    dry_run: bool,
    include_ontologies: bool,
    filter_subjects: Option<String>,
    output_format: &str,
) -> AtomicResult<()> {
    let sync_mode = mode
        .parse::<validate::SyncMode>()
        .map_err(|e| format!("Invalid sync mode: {}", e))?;
    let strategy = conflict_strategy
        .parse::<validate::ConflictStrategy>()
        .map_err(|e| format!("Invalid conflict strategy: {}", e))?;

    let filter = filter_subjects.map(|s| s.split(',').map(|x| x.trim().to_string()).collect());

    let options = validate::SyncOptions {
        mode: sync_mode,
        conflict_strategy: strategy,
        include_ontologies,
        dry_run,
        filter_subjects: filter,
        ..Default::default()
    };

    println!(
        "{}",
        format!(
            "Synchronizing servers:\n  Source: {}\n  Target: {}\n  Mode: {:?}\n  Strategy: {:?}{}",
            source_url,
            target_url,
            sync_mode,
            strategy,
            if dry_run { "\n  [DRY RUN]" } else { "" }
        )
        .blue()
        .bold()
    );

    match validate::sync_servers(source_url, target_url, agent_secret, options) {
        Ok(report) => {
            if output_format == "json" {
                let json = serde_json::to_string_pretty(&report)
                    .map_err(|e| format!("Failed to serialize report: {}", e))?;
                println!("{}", json);
            } else {
                println!("\n{}", "Sync Report".bold().underline());
                println!(
                    "Status: {}",
                    if report.success {
                        "SUCCESS".green().bold()
                    } else {
                        "FAILED".red().bold()
                    }
                );

                println!("\n{}", "Results:".bold());
                println!(
                    "  Resources Created: {}",
                    report.resources_created.to_string().green()
                );
                println!(
                    "  Resources Updated: {}",
                    report.resources_updated.to_string().yellow()
                );
                println!(
                    "  Resources Deleted: {}",
                    report.resources_deleted.to_string().red()
                );

                if !report.conflicts.is_empty() {
                    println!(
                        "\n{}",
                        format!("Conflicts ({}):", report.conflicts.len())
                            .yellow()
                            .bold()
                    );
                    for (i, conflict) in report.conflicts.iter().take(10).enumerate() {
                        println!("  {}. {}", i + 1, conflict.subject);
                        println!("     Property: {}", conflict.property);
                        println!(
                            "     Resolution: {:?}",
                            conflict.resolution.as_ref().unwrap_or(&validate::types::ConflictResolution::Manual)
                        );
                    }
                    if report.conflicts.len() > 10 {
                        println!("  ... and {} more", report.conflicts.len() - 10);
                    }
                }

                if !report.errors.is_empty() {
                    println!(
                        "\n{}",
                        format!("Errors ({}):", report.errors.len()).red().bold()
                    );
                    for (i, error) in report.errors.iter().take(10).enumerate() {
                        println!("  {}. {}: {}", i + 1, error.subject, error.error);
                    }
                    if report.errors.len() > 10 {
                        println!("  ... and {} more", report.errors.len() - 10);
                    }
                }

                if !report.warnings.is_empty() {
                    println!("\n{}", "Warnings:".yellow().bold());
                    for warning in &report.warnings {
                        println!("  - {}", warning);
                    }
                }

                if dry_run {
                    println!(
                        "\n{}",
                        "This was a dry run. No changes were made."
                            .yellow()
                            .bold()
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Sync failed: {}", e).red());
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Test connectivity to a server
fn test_connection_command(server_url: &str, agent_secret: Option<String>) -> AtomicResult<()> {
    println!(
        "{}",
        format!("Testing connection to: {}", server_url)
            .blue()
            .bold()
    );

    match validate::test_server_connection(server_url, agent_secret) {
        Ok(info) => {
            println!("\n{}", "Connection Test Results".bold().underline());
            println!("Server URL: {}", info.url);
            println!(
                "Status: {}",
                if info.reachable {
                    "REACHABLE".green().bold()
                } else {
                    "UNREACHABLE".red().bold()
                }
            );
            println!("Latency: {}ms", info.latency_ms.to_string().cyan());
            println!("Response Size: {} bytes", info.response_size);

            if info.latency_ms < 100 {
                println!("{}", "  Excellent connectivity!".green());
            } else if info.latency_ms < 500 {
                println!("{}", "  Good connectivity".green());
            } else if info.latency_ms < 2000 {
                println!("{}", "  Moderate latency".yellow());
            } else {
                println!("{}", "  High latency - may affect sync performance".red());
            }
        }
        Err(e) => {
            eprintln!("{}", format!("Connection test failed: {}", e).red());
            std::process::exit(1);
        }
    }

    Ok(())
}

/// List all bookmarks
fn list(context: &mut Context) {
    let mut string = String::new();
    for (shortname, url) in context.mapping.lock().unwrap().clone().into_iter() {
        string.push_str(&format!(
            "{0: <15}{1: <10} \n",
            shortname.blue().bold(),
            url
        ));
    }
    println!("{}", string)
}

/// Validates the store
fn validate(context: &mut Context) {
    let reportstring = context.store.validate().to_string();
    println!("{}", reportstring);
}

pub type CLIResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;
