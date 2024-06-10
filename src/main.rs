use clap::Parser;

use omnischedule::solver::{MiniMax, Solver};
use omnischedule::{Project, ProjectConfig, ResourceConfig};

#[derive(Parser, Debug)]
//#[serde(default)]
pub struct CliArgs {
    /// The configuration YAML file for the project
    project_file: String,

    /// Add an additional resource with the form <name>=<count>
    #[arg(short, long)]
    add_resource: Vec<ResourceConfig>,

    #[arg(default_value = "100")]
    search_depth: usize,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            project_file: "".to_string(),
            add_resource: vec![],
            search_depth: 100,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = CliArgs::try_parse()?;
    let project_file = std::fs::read_to_string(args.project_file)?;
    let mut config: ProjectConfig = toml::from_str(&project_file)?;
    for resource in args.add_resource.into_iter() {
        config.add_resource(&resource.name, resource.count);
    }

    let project = Project::try_from(config)?;
    let solution = MiniMax::new(5).solve(&project)?;
    solution.print();

    println!("Hello, from omnischedule!");
    Ok(())
}
