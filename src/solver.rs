use crate::project::{Project, Solution};
use anyhow::Context;
use itertools::Itertools;
use rayon::prelude::*;

pub trait Solver {
    type Error;
    /// Get the most optimal schedule for the given project.
    fn solve(&self, project: &Project) -> Result<Solution, Self::Error>;

    /// Determines how optimal the solurion is. The higher the score, the more optimal.
    fn score(&self, solution: &Solution) -> f64;
}

/// Use the minimax algorithm to find the most optimal schedule.
#[derive(Copy, Clone, Debug, Default)]
pub struct MiniMax {
    search_depth: usize,
}

impl Solver for MiniMax {
    type Error = anyhow::Error;

    fn solve(&self, project: &Project) -> anyhow::Result<Solution> {
        let mut solution = Solution::new(&project.resources);
        let mut max_score = 0.0;

        // each iteration of the loop we should be allocating a single task so by the end of
        // iterating all the tasks in the project all tasks should be allocated
        for i in 0..project.tasks.len() {
            let (next_score, mut next_solutions) = self.par_minimax(project, &solution, 5)?;
            anyhow::ensure!(
                next_score >= max_score,
                "next score lower than current score"
            );
            max_score = next_score;
            solution = next_solutions.pop()
                .ok_or_else(|| {
                    anyhow::anyhow!("no solutions?").context(anyhow::anyhow!(
                        "last working solution:\n{}",
                        solution.timeline()
                    ))
                })?;
            println!("Scheduled {} tasks", i + 1);
        }
        let solution_task_count = solution.task_entries().count();
        if solution_task_count == project.tasks.len() {
            Ok(solution.clone())
        } else {
            solution.print();
            anyhow::bail!("Not all tasks got scheduled. Tasks: {solution_task_count}");
        }
    }

    fn score(&self, solution: &Solution) -> f64 {
        let work = solution.work() as f64;
        let duration = solution.duration() as f64;
        // Bit of subtlety here. Because we add work and not task count we effectively promote the
        // algorithm to place the "big rocks" first. This is useful because longer tasks tend to
        // carry more uncertainty and risk.
        let result = work + (work / duration);
        if result.is_nan() {
            return 0.0;
        }
        result
    }
}

impl MiniMax {
    pub fn new(search_depth: usize) -> Self {
        MiniMax { search_depth }
    }

    fn inner_minimax(
        &self,
        project: &Project,
        root_solution: &Solution,
        depth: usize,
    ) -> anyhow::Result<(f64, Vec<Solution>)> {
        if depth == 0 {
            return Ok((self.score(root_solution), vec![root_solution.clone()]));
        }
        let combinations = itertools::iproduct!(
            project.resources.iter().map(|r| r.id),
            project.unscheduled_tasks(root_solution),
        );
        let mut best_score = self.score(root_solution);
        let mut best_solution = vec![root_solution.clone()];
        let mut errors: Vec<anyhow::Error> = Vec::new();
        for (resource, task) in combinations {
            let mut child = root_solution.clone();
            match child.push_task(resource, task) {
                Err(e) => {
                    errors.push(
                        anyhow::anyhow!("Task {}. {} is unschedulable", task.id.0, task.name)
                            .context(e),
                    );
                    continue;
                }
                Ok(_start_date) => (),
            };
            match self.inner_minimax(project, &child, depth - 1) {
                Ok((score, _winners)) if score > best_score => {
                    best_score = score;
                    best_solution = vec![child];
                }
                Ok((score, _winners)) if score == best_score => {
                    best_solution.push(child);
                }
                Ok(_) => continue,
                Err(e) => {
                    dbg!(e);
                    continue;
                }
            }
        }
        Ok((best_score, best_solution))
    }

    fn par_minimax(
        &self,
        project: &Project,
        root_solution: &Solution,
        depth: usize,
    ) -> anyhow::Result<(f64, Vec<Solution>)> {
        if depth == 0 {
            return Ok((self.score(root_solution), vec![root_solution.clone()]));
        }
        let combinations = itertools::iproduct!(
            project.resources.iter().map(|r| r.id),
            project.unscheduled_tasks(root_solution),
        );
        let results: Vec<anyhow::Result<(f64, Solution)>> = combinations
            .par_bridge()
            .map(|(resource, task)| {
                let mut child = root_solution.clone();
                child.push_task(resource, task).map_err(|e| {
                    anyhow::anyhow!("Task {}. {} is unschedulable", task.id.0, task.name).context(e)
                })?;
                let (score, _) = self.inner_minimax(project, &child, depth - 1)?;
                Ok((score, child))
            })
            .collect();
        if results.iter().all(|r| r.is_err()) {
            let errors = results.into_iter().filter_map(|r| match r {
                Ok(_) => None,
                Err(e) => Some(e),
            });
            let mut error = anyhow::anyhow!("all children failed\n").context(anyhow::anyhow!(
                "last working solution:\n{}",
                root_solution.timeline()
            ));
            for e in errors.into_iter() {
                error = error.context(e);
            }
            return Err(error);
        }
        let children: Vec<(f64, Solution)> = results.into_iter().filter_map(|r| r.ok()).collect();
        let best_score = children
            .iter()
            .map(|(score, _)| *score)
            .reduce(f64::max)
            .ok_or(anyhow::anyhow!("no scores?"))?;
        let best_solutions = children
            .into_iter()
            .filter(|(score, _winners)| *score == best_score)
            .map(|(_score, solutions)| solutions)
            .collect_vec();
        Ok((best_score, best_solutions))
    }
}

/// Determine if a given slice is ordered according to the key function
fn is_sorted<T, O, P>(l: &[T], mut key: P) -> bool
where
    O: PartialOrd,
    P: FnMut(&T) -> O,
{
    let mut l = l;
    for _ in l.iter() {
        match l {
            [] => return true,
            [_] => return true,
            [a, b, ..] => {
                if key(b) < key(a) {
                    return false;
                }
                l = &l[1..];
            }
        }
    }
    true
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ProjectConfig;

    #[test]
    fn solve_with_minimax() {
        let example_config = r#"
resources = [
  {name = "10x"},
  {name = "Ops"},
  {name = "Sr"},
  {name = "Mid"},
]

[sizes]
S = 10
M = 30
L = 90

[[tasks]]
name = "Task 1"
resource = {in = ["10x"]}
size = "S"
dependencies = ["Task 2", "Task 3"]

[[tasks]]
name = "Task 2"
resource = {in = ["Sr", "Mid"]}
size = "M"
dependencies = []

[[tasks]]
name = "Task 3"
resource = {in = ["Sr", "Mid"]}
size = "L"
dependencies = []
"#;
        let config: ProjectConfig =
            toml::from_str(example_config).expect("failed to parse example config file");
        let project = Project::try_from(config).unwrap();
        let solution = MiniMax::new(3).solve(&project).unwrap();
        solution.print();
        solution.csv_table().print_tty(false).unwrap();
        assert_eq!(project.tasks.len(), 3);
        assert_eq!(solution.task_entries().count(), 3);
        assert_eq!(solution.work(), 130);
        assert_eq!(solution.duration(), 100);
    }

    #[test]
    fn test_is_sorted() {
        let ordered: Vec<u16> = Vec::from_iter(0..100);
        let unordered = [3, 4, 1, 9, 8];
        assert!(is_sorted(ordered.as_slice(), |i| *i));
        assert!(!is_sorted(unordered.as_slice(), |i| *i));
    }
}
