use std::collections::HashMap;

pub type Name = &'static str;
pub type Skill = &'static str;
pub type Segment = &'static str;

// A character is, really, just the sum of their tasks.
// Sometimes we want to replace their components, which is done implicitly
// whenever we run a new task with the same subtype and name.
//
// The simulator runs whenever At is used, and will run to completion once the
// task list is exhausted.
#[derive(Debug)]
pub enum Task {
    At {
        date: chrono::NaiveDate,
    },
    Baseline {
        name: Name,
        skills: HashMap<Skill, f64>,
    },
    Schedule {
        name: Name,
        segment: HashMap<Segment, f64>,
    },
    SafetyLimit {
        name: Name,
        limit: HashMap<Skill, f64>,
    },
    ScheduleLimit {
        name: Name,
        limit: HashMap<Segment, Vec<Skill>>,
    },
    Overlap {
        name: Name,
        when: Vec<Overlap>,
    },
    Target {
        name: Name,
        target: HashMap<Skill, f64>,
    },
}

#[derive(Debug)]
pub struct Person {
    pub name: Name,
    pub skills: HashMap<Skill, f64>,
    pub schedule: HashMap<Segment, f64>,
    pub safety_limit: HashMap<Skill, f64>,
    pub schedule_limit: HashMap<Segment, Vec<Skill>>,
    pub overlap: Vec<Overlap>,
    pub target: HashMap<Skill, f64>,
}

impl Person {
    pub fn new(name: Name, skills: HashMap<Skill, f64>) -> Self {
        Self {
            name,
            skills,
            schedule: HashMap::new(),
            safety_limit: HashMap::new(),
            schedule_limit: HashMap::new(),
            overlap: vec![],
            target: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Overlap {
    pub combo: Vec<Skill>,
    pub bonus: f64,
}
