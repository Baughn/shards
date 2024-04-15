use chrono::NaiveDate;
use good_lp::{constraint, default_solver, variable, variables, Expression, Solution, SolverModel};
use log::{debug, info};
use maplit::hashmap;
use std::collections::HashMap;

mod types;
use crate::types::*;

fn main() {
    env_logger::init();

    let start = NaiveDate::from_ymd_opt(2009, 09, 01).unwrap();
    let schedule: Vec<Task> = vec![
        Task::Baseline {
            name: "Amu",
            skills: hashmap! {
                "Dreamwalking" => 1.0,
                "Illusion" => 1.0,
                "Integrity" => 2.0,
                "Lore" => 1.0,
            },
        },
        Task::Schedule {
            name: "Amu",
            segment: hashmap! {
                "School" => 1.0,
                "Afternoon" => 2.0,
                "Evening" => 1.0,
                "Sleep" => 0.5,
            },
        },
        Task::SafetyLimit {
            name: "Amu",
            limit: hashmap! {
                "Integrity" => 2.0,
            },
        },
        Task::ScheduleLimit {
            name: "Amu",
            limit: hashmap! {
                "School" => vec!["Illusion", "Lore"],
                "Sleep" => vec!["Dreamwalking", "Integrity"],
            },
        },
        Task::Overlap {
            name: "Amu",
            when: vec![
                Overlap {
                    combo: vec!["Illusion", "Dreamwalking"],
                    bonus: 1.25,
                },
                Overlap {
                    combo: vec!["Dreamwalking", "Integrity"],
                    bonus: 1.25,
                },
                Overlap {
                    combo: vec!["Lore", "Integrity"],
                    bonus: 1.1,
                },
            ],
        },
        Task::Target {
            name: "Amu",
            target: hashmap! {
                "Dreamwalking" => 2.0,
                "Illusion" => 2.0,
                "Integrity" => 3.0,
                "Lore" => 1.5,
            },
        },
    ];

    // Run the schedule.
    log::debug!("Schedule: {:?}", schedule);
    let mut now = start;
    let mut persons: HashMap<&str, Person> = hashmap! {};
    for task in schedule {
        match task {
            Task::At { date } => {
                if date <= now {
                    panic!("Cannot go back in time: {} < {}", date, now);
                }
                while now < date {
                    simulate_day(&mut persons, now);
                    now = now.succ_opt().unwrap();
                }
            }
            Task::Baseline { name, skills } => {
                if persons.contains_key(name) {
                    panic!("Person already exists: {}", name);
                }
                persons.insert(name, Person::new(name, skills));
            }
            Task::Schedule { name, segment } => {
                let person = persons
                    .get_mut(name)
                    .unwrap_or_else(|| panic!("Person not found: {}", name));
                person.schedule = segment;
            }
            Task::SafetyLimit { name, limit } => {
                let person = persons
                    .get_mut(name)
                    .unwrap_or_else(|| panic!("Person not found: {}", name));
                person.safety_limit = limit;
            }
            Task::ScheduleLimit { name, limit } => {
                let person = persons
                    .get_mut(name)
                    .unwrap_or_else(|| panic!("Person not found: {}", name));
                person.schedule_limit = limit;
            }
            Task::Overlap { name, when } => {
                let person = persons
                    .get_mut(name)
                    .unwrap_or_else(|| panic!("Person not found: {}", name));
                person.overlap = when;
            }
            Task::Target { name, target } => {
                let person = persons
                    .get_mut(name)
                    .unwrap_or_else(|| panic!("Person not found: {}", name));
                person.target = target;
            }
        }
    }
    // At the end of the schedule.
    // Run the simulator until no-one has any skill-up targets left.
    while persons.iter().any(|(_, person)| person.target.len() > 0) {
        simulate_day(&mut persons, now);
        now = now.succ_opt().unwrap();
        return;
    }
    info!("Simulation complete.");
}

fn simulate_day(persons: &mut HashMap<&str, Person>, date: NaiveDate) {
    info!("Date: {}", date);
    for (_, person) in persons.iter_mut() {
        simulate_person(person);
    }
}

fn simulate_person(person: &mut Person) {
    let mut vars = variables!();
    let mut constraints = vec![];

    // Define effective training times for each skill.
    // These are basically the output of the solver.
    let mut effective_training_time: HashMap<Skill, _> = hashmap! {};
    for (skill, _) in &person.target {
        let var = vars.add(variable().min(0.0));
        effective_training_time.insert(skill, var);
    }

    // Define training time variables for each skill, for each segment.
    // These represent literal time spent.
    let mut training_time: HashMap<Segment, HashMap<Skill, _>> = hashmap! {};
    // And total training time for each skill.
    let mut total_training_time: HashMap<Skill, _> = hashmap! {};
    for (segment, _) in &person.schedule {
        // Define the total time used during this segment, too.
        // This is the sum of all the training times.
        let mut total_time = Expression::from(0.0);
        let mut skill_vars: HashMap<Skill, _> = hashmap! {};
        for (skill, _) in &person.skills {
            let var = vars.add(variable().min(0.0));
            skill_vars.insert(skill, var);
            // Sum into per-segment total.
            total_time += var;
            // And per-skill total.
            let ttt = total_training_time
                .entry(skill)
                .or_insert_with(|| Expression::from(0.0));
            *ttt += var;
        }
        training_time.insert(segment, skill_vars);
        // Constraint: Total time used during this segment must be less than or equal to the segment's length.
        constraints.push(constraint!(total_time <= person.schedule[segment]));
    }
    // Constraint: Total time spent on each skill must be less than or equal to the safety limit, where defined.
    for (skill, limit) in &person.safety_limit {
        if let Some(ttt) = total_training_time.get(skill) {
            constraints.push(ttt.clone().leq(*limit));
        }
    }

    // Define the "overlapped" training times.
    // Each skill comes in a non-overlapped version, and may also come in overlapped versions
    // if the Person has a defined bonus for that combination.
    //
    // To map this up to training time and effective training time, we get two distinct sets
    // of constraints:
    //
    // 1. The training time for a skill is equal to the sum of the overlapped training times for that skill,
    //    except that each overlapped training time is divided by the number of skills in the combination.
    //
    // 2. The effective training time for a skill is equal to the sum of the overlapped training times for that skill,
    //    where each overlapped training time is multiplied by the bonus for that combination and divided by
    //    the number of skills in the combination.
    //
    // All of this is done per-segment.
    let mut overlapped_training_time: HashMap<String, HashMap<String, _>> = hashmap! {};
    // The 'skill' here can be either the pure skill name, or a combination of skills separated with underscores.
    for (segment, _) in &person.schedule {
        let mut segment_overlapped_training_time: HashMap<String, _> = hashmap! {};
        for (skill, _) in &person.target {
            let var = vars.add(variable().min(0.0));
            segment_overlapped_training_time.insert(skill.to_string(), var);
        }
        for overlap in &person.overlap {
            let name = overlap.combo.join("_");
            let var = vars.add(variable().min(0.0));
            segment_overlapped_training_time.insert(name, var);
        }
        overlapped_training_time.insert(segment.to_string(), segment_overlapped_training_time);
    }
    // Define training time constraints:
    for (segment, _) in &person.schedule {
        for (skill, _) in &person.target {
            let mut total_overlapped_time =
                Expression::from(overlapped_training_time[*segment][*skill]);
            for overlap in &person.overlap {
                if overlap.combo.contains(skill) {
                    let name = overlap.combo.join("_");
                    total_overlapped_time +=
                        overlapped_training_time[*segment][&name] / overlap.combo.len() as f64;
                }
            }
            // 1. The training time for a skill is equal to the sum of the overlapped training times for that skill,
            constraints.push(total_overlapped_time.eq(total_training_time[skill].clone()));
        }
    }
    // Define effective training time constraints:
    for (segment, _) in &person.schedule {
        for (skill, _) in &person.target {
            let mut total_effective_overlapped_time =
                Expression::from(overlapped_training_time[*segment][*skill]);
            for overlap in &person.overlap {
                if overlap.combo.contains(skill) {
                    let name = overlap.combo.join("_");
                    total_effective_overlapped_time += overlapped_training_time[*segment][&name]
                        / overlap.combo.len() as f64
                        * overlap.bonus;
                }
            }
            // 2. The effective training time for a skill is equal to the sum of the overlapped training times for that skill,
            constraints.push(constraint!(
                effective_training_time[skill] == total_effective_overlapped_time
            ));
        }
    }

    // Define the objective variable: Maximize effective training time.
    let mut total_effective_training_time = Expression::from(0.0);
    for (_, var) in &effective_training_time {
        total_effective_training_time += var;
    }

    // And solve! Finally.
    let mut solution = vars
        .maximise(total_effective_training_time)
        .using(good_lp::default_solver);
    for constraint in constraints {
        solution = solution.with(constraint);
    }
    let result = solution.solve().expect("Expected *some* sort of solution!");

    // Print each and every variable, for debugging.
    for (skill, eff) in &effective_training_time {
        info!(
            "Skill: {}, Effective Training Time: {}",
            skill,
            result.value(*eff)
        );
    }
    for (segment, skills) in &training_time {
        for (skill, time) in skills {
            info!(
                "Segment: {}; {}, Time: {}",
                segment,
                skill,
                result.value(*time)
            );
        }
    }
    for (segment, skills) in &overlapped_training_time {
        for (skill, time) in skills {
            info!(
                "Combo: {}; {}, Time: {}",
                segment,
                skill,
                result.value(*time)
            );
        }
    }
}
