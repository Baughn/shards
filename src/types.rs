use std::collections::BTreeMap;

pub type Name = &'static str;
pub type Skill = &'static str;
pub type Segment = &'static str;

// This is required to give deterministic results.
const DEFAULT_PRIORITY_ORDER: &[Skill] = &["Integrity", "Dreamwalking", "Illusion", "Lore"];
// The low offset value ensures overlap bonuses aren't ignored.
const DEFAULT_PRIORITY_OFFSET: f32 = 0.0;

// A character is, really, just the sum of their tasks.
// Sometimes we want to replace their components, which is done implicitly
// whenever we run a new task with the same subtype and name.
//
// The simulator runs whenever At is used, and will run to completion once the
// task list is exhausted.
#[derive(Debug)]
#[allow(dead_code)]
pub enum Task {
    At {
        date: chrono::NaiveDate,
    },
    Baseline {
        name: Name,
        skills: BTreeMap<Skill, f32>,
    },
    Schedule {
        name: Name,
        segment: BTreeMap<Segment, f32>,
    },
    SafetyLimit {
        name: Name,
        limit: BTreeMap<Skill, f32>,
    },
    ScheduleLimit {
        name: Name,
        limit: BTreeMap<Segment, Vec<Skill>>,
    },
    Overlap {
        name: Name,
        when: Vec<Overlap>,
    },
    Target {
        name: Name,
        target: BTreeMap<Skill, f32>,
    },
}

#[derive(Debug)]
pub struct Person {
    pub name: Name,
    // This person's skills, training or not.
    pub skills: BTreeMap<Skill, f32>,
    // This person's schedule, in terms of segments and their duration.
    pub schedule: BTreeMap<Segment, f32>,
    // Limits to how much some skills can be trained per day.
    pub safety_limit: BTreeMap<Skill, f32>,
    // Limits to which skills can be trained in which segments.
    // Some segments have no limit, and are not listed here.
    pub schedule_limit: BTreeMap<Segment, Vec<Skill>>,
    // Overlap bonuses for training multiple skills at once.
    // This *includes* the trivial case of training a single skill.
    pub overlap: Vec<Overlap>,
    // Target values for any skill being trained.
    pub target: BTreeMap<Skill, Target>,
    // Skill prefereces for training; defines which skills are trained first,
    // and by how much they're preferred. 1.0 is neutral; lower is less.
    // A skill's presence in this map does not imply the person is even capable
    // of training it.
    pub preference: BTreeMap<Skill, f32>,
}

impl Person {
    pub fn new(name: Name, skills: BTreeMap<Skill, f32>) -> Self {
        // Generate a default preference map.
        // We start at 1.0, then just add the offset per-skill.
        let preference = DEFAULT_PRIORITY_ORDER
            .iter()
            .rev()
            .enumerate()
            .map(|(i, skill)| (*skill, 1.0 + i as f32 * DEFAULT_PRIORITY_OFFSET))
            .collect();

        Self {
            name,
            skills,
            schedule: BTreeMap::new(),
            safety_limit: BTreeMap::new(),
            schedule_limit: BTreeMap::new(),
            overlap: vec![],
            target: BTreeMap::new(),
            preference,
        }
    }
}

#[derive(Debug)]
pub struct Overlap {
    pub combo: Vec<Skill>,
    pub bonus: f32,
}

#[derive(Debug)]
pub struct Target {
    pub target_ranks: f32,
    pub hours_needed: f32,
}
