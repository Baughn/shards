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
        skills: HashMap<Skill, f32>,
    },
    Schedule {
        name: Name,
        segment: HashMap<Segment, f32>,
    },
    SafetyLimit {
        name: Name,
        limit: HashMap<Skill, f32>,
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
        target: HashMap<Skill, f32>,
    },
}

#[derive(Debug)]
pub struct Person {
    pub name: Name,
    // This person's skills, training or not.
    pub skills: HashMap<Skill, f32>,
    // This person's schedule, in terms of segments and their duration.
    pub schedule: HashMap<Segment, f32>,
    // Limits to how much some skills can be trained per day.
    pub safety_limit: HashMap<Skill, f32>,
    // Limits to which skills can be trained in which segments.
    // Some segments have no limit, and are not listed here.
    pub schedule_limit: HashMap<Segment, Vec<Skill>>,
    // Overlap bonuses for training multiple skills at once.
    // This *includes* the trivial case of training a single skill.
    pub overlap: Vec<Overlap>,
    pub target: HashMap<Skill, f32>,
}

impl Person {
    pub fn new(name: Name, skills: HashMap<Skill, f32>) -> Self {
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
    pub bonus: f32,
}
