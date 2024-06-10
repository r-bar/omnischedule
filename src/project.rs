use itertools::Itertools;
use prettytable::Table;
use prettytable::{format::FormatBuilder, format::LinePosition, format::LineSeparator, row, table};
use std::collections::HashMap;
use std::collections::HashSet;
use std::iter::repeat;
use std::str::FromStr;

type SizeMap = HashMap<String, usize>;

#[derive(serde::Deserialize)]
pub struct ProjectConfig {
    pub resources: Vec<ResourceConfig>,
    pub sizes: SizeMap,
    pub tasks: Vec<TaskConfig>,
}

impl ProjectConfig {
    fn resource_list(&self) -> Vec<Resource> {
        self.resources
            .iter()
            .flat_map(|r| repeat(&r.name).take(r.count))
            .enumerate()
            .map(|(id, name)| Resource {
                id: ResourceId(id),
                name: name.to_string(),
            })
            .collect()
    }

    pub fn add_resource(&mut self, name: &str, count: usize) {
        let resource = ResourceConfig {
            name: name.into(),
            count,
        };
        for existing_resource in self.resources.iter_mut() {
            if existing_resource.name == resource.name {
                existing_resource.count += resource.count;
                break;
            }
        }
        self.resources.push(resource);
    }
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Dependency {
    Name(String),
    Index(usize),
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SizeConfig {
    Name(String),
    Days(usize),
}

impl SizeConfig {
    fn days(&self, size_map: &SizeMap) -> Option<usize> {
        match self {
            SizeConfig::Days(days) => Some(*days),
            SizeConfig::Name(name) => size_map.get(name).copied(),
        }
    }
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskResourceConfig {
    In(HashSet<String>),
    NotIn(HashSet<String>),
}

impl Default for TaskResourceConfig {
    fn default() -> Self {
        Self::NotIn(HashSet::new())
    }
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct TaskConfig {
    pub name: String,
    #[serde(default)]
    pub resource: TaskResourceConfig,
    pub size: SizeConfig,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub(crate) id: TaskId,
    pub(crate) name: String,
    pub(crate) resource_options: Vec<ResourceId>,
    pub(crate) days: usize,
    pub(crate) dependencies: Vec<TaskId>,
}

impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl TryFrom<&ProjectConfig> for Vec<Task> {
    type Error = anyhow::Error;

    fn try_from(config: &ProjectConfig) -> std::prelude::v1::Result<Self, Self::Error> {
        let resource_list = config.resource_list();
        let mut tasks = Vec::with_capacity(config.tasks.len());
        for i in 0..config.tasks.len() {
            tasks.push(Task::try_from_project_config(i, config, &resource_list)?);
        }
        Ok(tasks)
    }
}

impl Task {
    fn try_from_project_config(
        index: usize,
        config: &ProjectConfig,
        resource_list: &[Resource],
    ) -> anyhow::Result<Self> {
        let task_config = config
            .tasks
            .get(index)
            .ok_or(anyhow::anyhow!("Task index out of bounds: {index}"))?;
        let days = task_config.size.days(&config.sizes).ok_or(anyhow::anyhow!(
            "Invalid size \"{:?}\" for task {}",
            task_config.size,
            index
        ))?;
        let mut dependencies = Vec::with_capacity(task_config.dependencies.len());
        for dep in task_config.dependencies.iter() {
            let i: usize = match dep {
                Dependency::Name(name) => match config.tasks.iter().position(|t| &t.name == name) {
                    Some(i) => i,
                    None => {
                        anyhow::bail!("Invalid dependency reference {:?} for task {}", dep, index)
                    }
                },
                Dependency::Index(i) => *i,
            };
            if i >= config.tasks.len() {
                anyhow::bail!("Invalid dependency reference {:?} for task {}", dep, index);
            }
            dependencies.push(TaskId(i));
        }
        let mut resource_options = Vec::with_capacity(resource_list.len());
        match &task_config.resource {
            TaskResourceConfig::In(list) => {
                for (i, resource) in resource_list.iter().enumerate() {
                    if list.contains(&resource.name) {
                        resource_options.push(ResourceId(i));
                    }
                }
            }
            TaskResourceConfig::NotIn(list) => {
                for (i, resource) in resource_list.iter().enumerate() {
                    if !list.contains(&resource.name) {
                        resource_options.push(ResourceId(i))
                    }
                }
            }
        }
        anyhow::ensure!(
            !resource_options.is_empty(),
            "No resources match task {} resource declaration {:?}",
            index,
            task_config.resource,
        );
        Ok(Task {
            id: TaskId(index),
            name: task_config.name.to_string(),
            resource_options,
            days,
            dependencies,
        })
    }
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ResourceConfig {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Eq)]
pub struct Resource {
    pub id: ResourceId,
    pub name: String,
}

impl PartialEq for Resource {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            name: String::default(),
            count: 1,
        }
    }
}

impl FromStr for ResourceConfig {
    type Err = anyhow::Error;

    /// From "<name>=<count>"
    fn from_str(s: &str) -> std::prelude::v1::Result<Self, Self::Err> {
        s.split_once('=')
            .and_then(|(name, count_str)| {
                if let Ok(count) = count_str.parse() {
                    Some(ResourceConfig {
                        name: name.into(),
                        count,
                    })
                } else {
                    None
                }
            })
            .ok_or(anyhow::anyhow!("Invalid resource specification"))
    }
}

impl TryFrom<ProjectConfig> for Project {
    type Error = anyhow::Error;

    fn try_from(value: ProjectConfig) -> std::prelude::v1::Result<Self, Self::Error> {
        Ok(Project {
            tasks: (&value).try_into()?,
            resources: value.resource_list(),
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub struct UnscheduledTasks<'a> {
    project: &'a Project,
    solution: &'a Solution,
    ptr: usize,
}

impl<'a> UnscheduledTasks<'a> {
    fn new(project: &'a Project, solution: &'a Solution) -> Self {
        Self {
            project,
            solution,
            ptr: 0,
        }
    }
}

impl<'a> Iterator for UnscheduledTasks<'a> {
    type Item = &'a Task;

    fn next(&mut self) -> Option<Self::Item> {
        while self.ptr < self.project.tasks.len() {
            let task = &self.project.tasks[self.ptr];
            self.ptr += 1;
            let task_in_solution = self
                .solution
                .task_queues
                .iter()
                .any(|q| q.tasks.iter().any(|entry| entry.id == task.id));
            if !task_in_solution {
                return Some(task);
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct Project {
    pub(crate) resources: Vec<Resource>,
    pub(crate) tasks: Vec<Task>,
}

impl<'a> Project {
    pub fn unscheduled_tasks(&'a self, solution: &'a Solution) -> UnscheduledTasks<'a> {
        UnscheduledTasks::new(self, solution)
    }

    pub fn get_task(&self, task_id: TaskId) -> &Task {
        &self.tasks[task_id.0]
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceId(pub usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(pub usize);

#[derive(Debug, Clone)]
pub struct TaskQueueEntry {
    start: usize,
    days: usize,
    id: TaskId,
    label: String,
}

impl TaskQueueEntry {
    pub fn end(&self) -> usize {
        self.start + self.days
    }
}

#[derive(Clone, Debug)]
pub struct TaskQueue {
    pub assignee: Resource,
    pub tasks: Vec<TaskQueueEntry>,
}

impl TaskQueue {
    /// Create a new, empty queue for the assignee.
    pub fn new(assignee: &Resource) -> Self {
        Self {
            assignee: assignee.clone(),
            tasks: vec![],
        }
    }

    /// The number of days after the start of the timeline the last work is complete for this
    /// queue.
    pub fn duration(&self) -> usize {
        if self.tasks.is_empty() {
            0
        } else {
            let entry = &self.tasks[self.tasks.len() - 1];
            entry.start + entry.days
        }
    }

    /// Returns the amount of work performed in the queue. In a completely packed queue this will
    /// equal the duration.
    pub fn work(&self) -> usize {
        self.tasks.iter().map(|entry| entry.days).sum()
    }

    /// Add a new event to the end of the task queue. Does not fill holes.
    pub fn push(&mut self, task: &Task) {
        self.tasks.push(TaskQueueEntry {
            start: self.duration(),
            id: task.id,
            days: task.days,
            label: task.name.clone(),
        });
    }

    /// Print the timeline representation for this task queeue.
    pub fn diagram(&self, day_width: f64) -> String {
        let width = (self.duration() as f64 * day_width).floor() as usize;
        let mut buffer: Vec<char> = std::iter::repeat(' ').take(width).collect();
        for entry in self.tasks.iter() {
            let block_width = (entry.days as f64 * day_width).floor() as usize;
            let start_index = (entry.start as f64 * day_width).floor() as usize;
            let end_index = start_index + block_width;
            let max_title_width = block_width.saturating_sub(2);
            let title = format!("{}. {}", &entry.id.0, &entry.label);
            let mut block = Vec::with_capacity(block_width);
            block.push('|');
            block.extend(
                title
                    .chars()
                    .chain(std::iter::repeat(' '))
                    .take(max_title_width),
            );
            block.push('|');
            if start_index == end_index {
                continue;
            }
            buffer[start_index..end_index].copy_from_slice(&block);
        }
        buffer.into_iter().collect()
    }

    pub fn insert(&mut self, start_day: usize, task: &Task) {
        self.tasks.push(TaskQueueEntry {
            id: task.id,
            label: task.name.clone(),
            start: start_day,
            days: task.days,
        });
        self.tasks.sort_by_key(|entry| entry.start);
    }
}

#[derive(Debug, Clone)]
pub struct Solution {
    pub task_queues: Vec<TaskQueue>,
}

impl Solution {
    pub fn new(resources: &[Resource]) -> Self {
        let mut task_queues = Vec::with_capacity(resources.len());
        task_queues.extend(resources.iter().map(TaskQueue::new));
        Solution { task_queues }
    }

    /// The time between starting work and the last task being complete
    pub fn duration(&self) -> usize {
        self.task_queues.iter().map(|q| q.duration()).max().unwrap()
    }

    /// The number of engineer days worked in this solution
    pub fn work(&self) -> usize {
        self.task_queues.iter().map(|q| q.work()).sum()
    }

    pub fn timeline(&self) -> Table {
        let day_width: f64 = 0.2;
        let mut t = table!(["Assignee", "Timeline"]);
        let format = FormatBuilder::new()
            .column_separator('#')
            .borders('#')
            .separator(LinePosition::Top, LineSeparator::new('#', '#', '#', '#'))
            .separator(LinePosition::Bottom, LineSeparator::new('#', '#', '#', '#'))
            .padding(1, 1)
            .build();
        t.set_format(format);
        for queue in self.task_queues.iter() {
            t.add_row(row![queue.assignee.name, queue.diagram(day_width)]);
        }
        t
    }

    /// Print a table representing the solution timeline for each assignee
    pub fn print(&self) {
        self.timeline().print_tty(false).unwrap();
    }

    /// Return the tasks contained in the solution
    pub fn task_entries(&self) -> impl Iterator<Item = &TaskQueueEntry> + '_ {
        self.task_queues.iter().flat_map(|q| &q.tasks)
    }

    /// Return the day on which the last dependency of the task is complete.
    pub fn dependencies_complete(&self, task: &Task) -> Option<usize> {
        self.task_entries()
            .filter(|entry| task.dependencies.contains(&entry.id))
            .map(|entry| entry.end())
            .max()
    }

    /// Adds a task to the end of the schedule of the given resource if all task requirements are satisfied.
    /// Returns the start date for the task pushed.
    pub fn push_task(&mut self, resource_id: ResourceId, task: &Task) -> anyhow::Result<usize> {
        let assignee = &self.task_queues[resource_id.0].assignee;
        anyhow::ensure!(assignee.id == resource_id, "assignee mismatch");
        if !task.resource_options.contains(&resource_id) {
            anyhow::bail!(
                "invalid resource assignment {:?} for {:?} {:?}",
                assignee,
                task.id,
                task.name
            );
        }
        let queue = &self.task_queues[resource_id.0];
        let start_date = queue
            .duration()
            .max(self.dependencies_complete(task).unwrap_or(0));
        for dep in task.dependencies.iter() {
            match self.task_entries().find(|e| e.id == *dep) {
                Some(entry) if entry.end() > start_date => {
                    anyhow::bail!(
                        "{:?} {:?} cannot be scheduled before the completion of the dependent task {:?}",
                        task.id,
                        task.name,
                        dep
                    )
                }
                None => anyhow::bail!(
                    "not all dependencies have been scheuled yet for {:?} {:?}",
                    task.id,
                    task.name
                ),
                Some(entry) => {
                    //dbg!(dep, entry, task, start_date);
                },
            }
        }
        let queue = &mut self.task_queues[resource_id.0];
        queue.insert(start_date, task);
        Ok(start_date)
    }

    pub fn csv_table(&self, start_date: chrono::NaiveDate) -> Table {
        let mut table = Table::new();
        table.add_row(row!["Resource", "Task", "Start", "End"]);
        for queue in self.task_queues.iter() {
            for entry in queue.tasks.iter() {
                let entry_start_date = start_date + chrono::Duration::days(entry.start as i64);
                let entry_end_date = start_date + chrono::Duration::days(entry.end() as i64);
                table.add_row(row![
                    queue.assignee.name,
                    entry.label,
                    entry_start_date,
                    entry_end_date,
                ]);
            }
        }
        table
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn example_is_a_valid_project() {
        let example = include_str!("../example.toml");
        let config: ProjectConfig = toml::from_str(example).expect("failed to parse example config file");
        Project::try_from(config).unwrap();
    }

    #[test]
    fn test_task_queue() {
        let mut queue = TaskQueue::new(&Resource {
            id: ResourceId(0),
            name: "foo".into(),
        });
        assert_eq!(queue.duration(), 0);
        assert_eq!(queue.work(), 0);
        let task1 = Task {
            id: TaskId(1),
            days: 30,
            name: "1".into(),
            resource_options: vec![],
            dependencies: vec![],
        };
        let task2 = Task {
            id: TaskId(2),
            days: 60,
            name: "2".into(),
            resource_options: vec![],
            dependencies: vec![],
        };
        queue.push(&task1);
        assert_eq!(queue.duration(), 30);
        assert_eq!(queue.work(), 30);
        queue.push(&task2);
        assert_eq!(queue.duration(), 90);
        assert_eq!(queue.work(), 90);
    }

    #[test]
    fn test_solution() {
        let task1 = Task {
            id: TaskId(1),
            days: 30,
            name: "1".into(),
            resource_options: vec![],
            dependencies: vec![],
        };
        let task2 = Task {
            id: TaskId(2),
            days: 60,
            name: "2".into(),
            resource_options: vec![],
            dependencies: vec![],
        };
        let mut solution = Solution::new(&[
            Resource {
                id: ResourceId(0),
                name: "foo".into(),
            },
            Resource {
                id: ResourceId(1),
                name: "bar".into(),
            },
        ]);
        solution.task_queues[0].push(&task1);
        solution.task_queues[1].push(&task2);
        let task3 = Task {
            id: TaskId(2),
            days: 60,
            name: "2".into(),
            resource_options: vec![],
            dependencies: vec![],
        };
        let mut solution2 = solution.clone();
        solution2.task_queues[0].push(&task3);
        assert_eq!(solution2.duration(), 90);
        assert_eq!(solution2.work(), 150);
        assert_eq!(solution.duration(), 60);
        assert_eq!(solution.work(), 90);
    }
}
