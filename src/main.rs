use chrono::NaiveDate;
use lazy_static::lazy_static;
use log::{debug, info};
use lp_modeler::{
    constraint,
    dsl::*,
    solvers::{self, SolverTrait},
};
use maplit::{btreemap, btreeset};
use std::collections::{BTreeMap, BTreeSet};

mod types;
use crate::types::*;

lazy_static! {
    static ref ATTRIBUTES: BTreeSet<Skill> = btreeset! {
        "Strength", "Dexterity", "Stamina",
        "Charisma", "Manipulation", "Appearance",
        "Perception", "Intelligence", "Wits",
    };
    static ref ABILITIES: BTreeSet<Skill> = btreeset! {
        "Archery", "Athletics", "Awareness",
        "Brawl", "Bureaucracy", "Craft",
        "Dodge", "Integrity", "Investigation",
        "Larceny", "Linguistics", "Lore",
        "Martial Arts", "Medicine", "Melee",
        "Occult", "Performance", "Presence",
        "Resistance", "Ride", "Sail",
        "Socialize", "Stealth", "Survival",
        "Thrown", "War",
        "Firearms", "Driving",
    };
    static ref PSIONICS: BTreeSet<Skill> = btreeset! {
        "Dreamwalking", "Illusion",
    };
}

fn main() {
    env_logger::init();

    let start = NaiveDate::from_ymd_opt(2009, 09, 01).unwrap();
    println!("{}: Time begins", start);
    let schedule: Vec<Task> = vec![
        Task::Baseline {
            name: "Amu",
            skills: btreemap! {
                "Dreamwalking" => 1.0,
                "Illusion" => 1.0,
                "Integrity" => 2.0,
                "Lore" => 1.0,
            },
        },
        Task::Schedule {
            name: "Amu",
            segment: btreemap! {
                "School" => 1.0,
                "Afternoon" => 2.0,
                "Evening" => 1.0,
                "Sleep" => 0.5,
            },
        },
        Task::SafetyLimit {
            name: "Amu",
            limit: btreemap! {
                "Integrity" => 2.0,
            },
        },
        Task::ScheduleLimit {
            name: "Amu",
            limit: btreemap! {
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
            target: btreemap! {
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
    let mut persons: BTreeMap<&str, Person> = btreemap! {};
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
                persons.get_mut(name).unwrap().schedule = segment;
            }
            Task::SafetyLimit { name, limit } => {
                persons.get_mut(name).unwrap().safety_limit = limit;
            }
            Task::ScheduleLimit { name, limit } => {
                persons.get_mut(name).unwrap().schedule_limit = limit;
            }
            Task::Overlap { name, mut when } => {
                let person = persons.get_mut(name).unwrap();
                // Add the trivial 1-skill 'overlaps'.
                for skill in person.skills.keys() {
                    when.push(Overlap {
                        combo: vec![skill],
                        bonus: 1.0,
                    });
                }
                person.overlap = when;
            }
            Task::Target { name, target } => {
                let person = persons.get_mut(name).unwrap();
                let mut new_targets = btreemap! {};
                for (skill, target_ranks) in target {
                    new_targets.insert(
                        skill,
                        Target {
                            target_ranks,
                            hours_needed: effective_training_hours_needed(
                                skill,
                                person.skills[skill],
                            ),
                        },
                    );
                }
                person.target = new_targets;
            }
        }
    }
    // At the end of the schedule.
    // Run the simulator until no-one has any skill-up targets left.
    let mut sum_roi = 0.0;
    let mut days = 0;
    while persons.iter().any(|(_, person)| person.target.len() > 0) {
        sum_roi += simulate_day(&mut persons, now);
        days += 1;
        now = now.succ_opt().unwrap();
    }
    println!("Total ROI: {}, ROI/day: {}", sum_roi, sum_roi / days as f32);
    info!("Simulation complete.");
}

fn simulate_day(persons: &mut BTreeMap<&str, Person>, now: NaiveDate) -> f32 {
    info!("Date: {}", now);
    let mut sum_roi = 0.0;
    for (_, person) in persons.iter_mut() {
        let (total_roi, increment) = simulate_person(&now, person);
        sum_roi += total_roi;
        for (skill, effective_hours_trained) in increment {
            person.target.get_mut(skill).unwrap().hours_needed -= effective_hours_trained;
            if person.target[skill].hours_needed <= 0.0 {
                println!(
                    "{}: {} has reached target rank of {} for {}",
                    now, person.name, person.skills[skill], skill
                );
                person.target.remove(skill);
            }
        }
    }
    sum_roi
}

// Returns effective training hours for the day.
fn simulate_person(now: &NaiveDate, person: &Person) -> (f32, BTreeMap<Skill, f32>) {
    // Define problem variables.
    //
    // Total return on investment, aka. skill-up points -- one per skill.
    // This is the output.
    let mut roi: BTreeMap<Skill, LpContinuous> = btreemap! {};
    for skill in person.target.keys() {
        let name = format!("ROI_{}", skill);
        roi.insert(skill, LpContinuous::new(&name));
    }

    // The time spent on each skill, by skill.
    // This is used for the safety check.
    let mut invested_skill: BTreeMap<Skill, LpContinuous> = btreemap! {};
    for skill in person.target.keys() {
        let name = format!("skill_{}", skill);
        invested_skill.insert(skill, LpContinuous::new(&name));
    }

    // The time spent in each segment, by segment.
    let mut invested_seg: BTreeMap<Segment, LpContinuous> = btreemap! {};
    for seg in person.schedule.keys() {
        let name = format!("segment_{}", seg);
        invested_seg.insert(seg, LpContinuous::new(&name));
    }

    // The time spent on each skill *combo*, by segment and combo.
    // This is needed to calculate the overlap bonus, and is the primary
    // thing you can think of the solver as optimizing.
    let mut invested_seg_combo: BTreeMap<(Segment, Vec<Skill>), LpContinuous> = btreemap! {};
    for seg in person.schedule.keys() {
        for combo in person.overlap.iter() {
            let name = format!("combo_{}_{}", seg, combo.combo.join("_"));
            invested_seg_combo.insert((seg, combo.combo.clone()), LpContinuous::new(&name));
        }
    }

    // Define objective function: maximize the total return on investment.
    let mut problem = LpProblem::new(person.name, LpObjective::Maximize);
    for (skill, var) in roi.iter() {
        problem += var * person.preference[skill];
    }

    // Define constraints.
    // 1. Spent time cannot be negative, for any segment/combo or skill.
    for var in invested_skill
        .values()
        .chain(invested_seg.values())
        .chain(invested_seg_combo.values())
    {
        problem += constraint!(var >= 0.0);
    }
    // 2. Time spent from a segment must be less than the segment limit.
    for (seg, limit) in person.schedule.iter() {
        let var = invested_seg.get(seg).unwrap();
        problem += constraint!(var <= limit);
    }
    // 3. Time spent on a skill must be less than the skill's safety limit, if any.
    for (skill, limit) in person.safety_limit.iter() {
        if let Some(var) = invested_skill.get(skill) {
            problem += constraint!(var <= limit);
        }
    }
    // 4. Time spent on a skill equals the sum of time spent on each combo that includes it.
    for (skill, total) in invested_skill.iter() {
        // Subtract from the total all the time spent on combos that include this skill,
        // and we should get zero.
        let mut antisum = LpExpression::from(total);
        for ((_, combo), var) in invested_seg_combo.iter() {
            if combo.contains(skill) {
                antisum -= var;
            }
        }
        problem += antisum.equal(0.0);
    }
    // 5. Time spent in a segment equals the sum of time spent on each combo in it...
    //    multiplied by the size of the combo.
    for (seg, total) in invested_seg.iter() {
        // Same trick as above.
        let mut antisum = LpExpression::from(total);
        for ((c_seg, combo), var) in invested_seg_combo.iter() {
            if c_seg == seg {
                antisum -= var * combo.len() as f32;
            }
        }
        problem += antisum.equal(0.0);
    }
    // 6. Return on investment equals the sum of time spent on each combo that includes it,
    //    multiplied by the bonus for that combo.
    for (skill, total) in roi.iter() {
        // Same trick as above.
        let mut antisum = LpExpression::from(total);
        for ((_, combo), var) in invested_seg_combo.iter() {
            if combo.contains(skill) {
                // Yeah yeah, this is a bit inefficient, but it's not a big deal.
                let bonus = person
                    .overlap
                    .iter()
                    .find(|o| o.combo == *combo)
                    .unwrap()
                    .bonus;
                antisum -= var * bonus;
            }
        }
        problem += antisum.equal(0.0);
    }
    // 7. For segments that have limitations on what skills can be trained,
    //   the time spent on every combo must be zero EXCEPT if it only contains
    //   permitted skills.
    for (seg, allowed) in person.schedule_limit.iter() {
        debug!(
            "Checking segment {:?} with allowed skills {:?}",
            seg, allowed
        );
        let allowed: BTreeSet<Skill> = allowed.iter().cloned().collect();
        for ((c_seg, combo), var) in invested_seg_combo.iter() {
            if c_seg == seg {
                let combo_set: BTreeSet<Skill> = combo.iter().cloned().collect();
                // println!("  Checking combo {:?}", combo_set);
                if !allowed.is_superset(&combo_set) {
                    debug!("  Adding constraint: {:?} is not allowed.", combo_set);
                    problem += var.equal(0.0);
                }
            }
        }
    }
    // 8. In any event, don't put in more time than is needed.
    for (skill, target) in person.target.iter() {
        problem += constraint!(roi[skill] <= target.hours_needed);
    }

    // Solve the problem.
    let solver = solvers::MiniLpSolver::new();
    let solution = solver
        .run(&problem)
        .expect("Failed to find a training schedule.");
    debug!("Solution: {:?}", solution);

    // problem.write_lp("/dev/stdout").unwrap();

    // // Print the results...
    // println!("Total RoI:");
    // let mut total = 0.0;
    // for (skill, var) in roi.iter() {
    //     println!("  {}: {}", skill, solution.get_float(var));
    //     total += solution.get_float(var);
    // }
    // println!("  Total: {}", total);
    // println!("Time spent on skills:");
    // for (skill, var) in invested_skill.iter() {
    //     println!("  {}: {}", skill, solution.get_float(var));
    // }
    // println!("Time spent on segments:");
    // for (seg, var) in invested_seg.iter() {
    //     println!("  {}: {}", seg, solution.get_float(var));
    // }
    // println!("Time spent on combos:");
    // for ((seg, combo), var) in invested_seg_combo.iter() {
    //     let value = solution.get_float(var);
    //     if value != 0.0 {
    //         println!("  {} {}: {}", seg, combo.join("_"), value);
    //     }
    // }

    // Check for wasted time.
    let mut wasted_time = 0.0;
    for (seg, limit) in person.schedule.iter() {
        let var = invested_seg.get(seg).unwrap();
        let value = solution.get_float(var);
        if value < *limit {
            wasted_time += limit - value;
        }
    }
    if wasted_time > 0.0 {
        println!("{}: Wasted time: {}", now, wasted_time);
    }

    // Return the results.
    let mut results = BTreeMap::new();
    let mut total_roi = 0.0;
    for (skill, var) in roi.iter() {
        results.insert(*skill, solution.get_float(var));
        total_roi += solution.get_float(var);
    }
    (total_roi, results)
}

// Computes the number of effective training hours needed to reach a target rank.
// This depends on the type of skill and the current rank.
fn effective_training_hours_needed(skill: &str, current_rank: f32) -> f32 {
    const HOURS_PER_WEEK: f32 = 48.0;
    const WEEKS_PER_MONTH: f32 = 4.0;
    let current_rank = current_rank.floor();
    if current_rank <= 0.0 {
        return if ATTRIBUTES.contains(skill) {
            3.0 * HOURS_PER_WEEK * WEEKS_PER_MONTH
        } else if ABILITIES.contains(skill) {
            3.0 * HOURS_PER_WEEK
        } else if PSIONICS.contains(skill) {
            2.0 * HOURS_PER_WEEK
        } else {
            panic!("Unknown skill type: {}", skill);
        };
    } else {
        return if ATTRIBUTES.contains(skill) {
            current_rank * HOURS_PER_WEEK * WEEKS_PER_MONTH
        } else if ABILITIES.contains(skill) {
            current_rank * HOURS_PER_WEEK
        } else if PSIONICS.contains(skill) {
            current_rank * HOURS_PER_WEEK
        } else {
            panic!("Unknown skill type: {}", skill);
        };
    }
}
