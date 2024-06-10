use std::fs::File;
use std::io::BufWriter;
use std::time::Instant;
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

    /// The maximum depth of the search tree to use. Deep trees can take a long time to solve.
    #[arg(short = 'd', long, default_value = "5")]
    search_depth: usize,

    /// Output the solution to a CSV file
    #[arg(short, long)]
    output_csv: Option<String>,


    /// Calculate the dates of the solution using the given start date
    #[arg(short, long)]
    start_date: Option<String>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            project_file: "".to_string(),
            add_resource: vec![],
            search_depth: 100,
            output_csv: None,
            start_date: None,
        }
    }
}

impl CliArgs {
    pub fn parse_start_date(&self) -> anyhow::Result<chrono::NaiveDate> {
        match self.start_date {
            None => Ok(chrono::Local::now().naive_local().date()),
            Some(ref date) if date == "today" => {
                Ok(chrono::Local::now().naive_local().date())
            }
            Some(ref date) => {
                Ok(chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")?)
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = CliArgs::try_parse()?;
    let project_file = std::fs::read_to_string(&args.project_file)?;
    let mut config: ProjectConfig = toml::from_str(&project_file)?;
    let start_date = args.parse_start_date()?;
    for resource in args.add_resource.into_iter() {
        config.add_resource(&resource.name, resource.count);
    }

    let project = Project::try_from(config)?;
    let solver_start_time = Instant::now();
    let solution = MiniMax::new(args.search_depth).solve(&project)?;
    let solver_end_time = Instant::now();
    solution.print();
    if let Some(output_csv) = args.output_csv {
        let csv_file = File::create(output_csv)?;
        let mut writer = BufWriter::new(csv_file);
        solution.csv_table(start_date).to_csv(&mut writer)?;
    }
    println!("Solver run time: {:?}", solver_end_time - solver_start_time);
    println!("Project start date: {}", &start_date);
    println!("Solution completion time: {} days", solution.duration());
    println!("Solution completion date: {}", start_date + chrono::Duration::days(solution.duration() as i64));

    Ok(())
}
