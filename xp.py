# This program simulates XP allocation and training times for the characters of an Exalted campaign.
#
# Specifically: Given a list of events such as 'start of training', 'xp award' and anomalies, the program
# simulates the state of the character(s) day-by-day, and calculates the exact time it takes to train their
# skills. Output is a list of dates at which the character(s) have completed their training.
#
# Variant rule: Some skills gain bonuses to training time if they are trained simultaneously. This is
# also calculated on a day-by-day basis.
#
# Variant rule: Since most of the characters are going to school, training time is limited by a weekly
# schedule. A few skills can be trained during school hours, but most can only be trained in free time.

import datetime
from dataclasses import dataclass
from typing import Dict, List
import math


def Events():
    return [
        Start(datetime.date(2009, 10, 1)),
        Person(
            name='Amu',
            skills={
                'Dreamwalking': 1,
                'Illusion': 1,
                'Lore': 1,
                'Integrity': 2,
            },
            schedule=Schedule(
                person='Amu',
                hours_per_day=2),
            training=[],
            limits=[],
            bonuses=[],
            overlaps=[]
        ),
        Limit('Amu', 'Integrity', hours_per_day=1),  # More is unsafe.
        Bonus('Amu', 'Sleeping', ['Dreamwalking', 'Integrity'], 0.5),
        Overlap('Amu', ['Dreamwalking', 'Integrity'], overlap=0.25),
        # https://forums.sufficientvelocity.com/threads/123509/post-30043543
        Training('Amu', 'Dreamwalking', 2),
        Training('Amu', 'Illusion', 2),
        Training('Amu', 'Lore', 1.5),
        Simulate(),
    ]


# Constants:
ATTRIBUTES = ['Strength', 'Dexterity', 'Stamina', 'Charisma', 'Manipulation', 'Appearance', 'Perception', 'Intelligence', 'Wits']
ABILITIES = ['Firearms', 'Martial Arts', 'Melee', 'Thrown', 'War', 'Craft', 'Investigation', 'Larceny', 'Stealth', 'Survival', 'Athletics', 'Awareness', 'Dodge', 'Integrity', 'Performance', 'Presence', 'Resistance', 'Ride', 'Sail', 'Socialize', 'Bureaucracy', 'Linguistics', 'Lore', 'Occult']
# Anything that doesn't match either of the above should be psionics.
COST_NEW = {
    # Exalted assumes 40 hours training per week.
    # This is (xp, time in hours).
    'Ability': (3, 3*40),
    'Psionics': (2, 2*40),
}
COST_UPGRADE = {
    # Defined as the cost to upgrade from n.
    'Attribute': lambda n: (n*4, n*40*4),
    'Ability': lambda n: (n*2, n*2*40),
    'Psionics': lambda n: (n*1.5, n*1.5*40),
}


# Event types:
@dataclass
class Start:
    date: datetime.date

@dataclass
class Schedule:
    person: str
    hours_per_day: float

@dataclass
class Training:
    person: str
    skill: str
    target: float

@dataclass
class Limit:
    person: str
    skill: str
    hours_per_day: float

@dataclass
class Bonus:
    person: str
    condition: str
    skills: [str]
    hours_per_day: float

@dataclass
class Overlap:
    person: str
    skills: [str]
    overlap: float

@dataclass
class Simulate:
    until: datetime.date = None

@dataclass
class Person:
    name: str
    skills: Dict[str, float]
    schedule: Schedule
    training: List[Training]
    limits: List[Limit]
    bonuses: List[Bonus]
    overlaps: List[Overlap]

def simulate():
    people = {}
    training_hours_applied = {}
    training_hours_needed = {}
    training_xp_spent = {}
    now = None
    for event in Events():

        if isinstance(event, Start):
            now = event.date
            print(f'Time began at {now}.')

        elif isinstance(event, Person):
            print(f'{now}: {event.name} has joined the simulation.')
            people[event.name] = event
            training_hours_applied[event.name] = {}
            training_hours_needed[event.name] = {}
            training_xp_spent[event.name] = {}

        elif isinstance(event, Schedule):
            print(f'{now}: {people[event.person].name} has a new schedule: {event.hours_per_day} hours per day.')
            people[event.person].schedule = event

        elif isinstance(event, Limit):
            print(f'{now}: {event.person} has a new limit on {event.skill}: {event.hours_per_day} hours per day.')
            # Remove any existing training time limit on this skill.
            person = people[event.person]
            person.limits = [l for l in person.limits if l.skill != event.skill]
            person.limits.append(event)

        elif isinstance(event, Bonus):
            print(f'{now}: {event.person} has set a bonus for {event.skills} while {event.condition}')
            # Remove any existing bonus on this condition.
            person = people[event.person]
            person.bonuses = [b for b in person.bonuses if b.condition != event.condition]
            person.bonuses.append(event)

        elif isinstance(event, Training):
            print(f'{now}: {event.person} has started training in {event.skill} to rank {event.target}.')
            people[event.person].training.append(event)
            # Compute hours needed for training.
            # This depends on the type of skill being trained, so...
            if event.skill in ABILITIES:
                typ = 'Ability'
            elif event.skill in ATTRIBUTES:
                typ = 'Attribute'
            else:
                typ = 'Psionics'
            # First, is this a new skill or an upgrade?
            current_rank = people[event.person].skills.get(event.skill, 0)
            if current_rank == 0:
                xp, hours = COST_NEW[typ]
            else:
                xp, hours = COST_UPGRADE[typ](current_rank)
            # Make sure we only attempt to go up one rank at a time,
            # as the calculation above would misfire otherwise.
            assert event.target - current_rank <= 1, f'Cannot train {event.skill} from {current_rank} to {event.target} in one go.'
            # Fractional upgrades are allowed, and are lerped.
            if event.target - current_rank < 1:
                xp *= event.target - current_rank
                hours *= event.target - current_rank
            print(f'  This will cost {xp} xp and take {hours} hours.')
            # Ok, good.
            training_hours_needed[event.person].setdefault(event.skill, 0)
            training_hours_applied[event.person].setdefault(event.skill, 0)
            training_xp_spent[event.person].setdefault(event.skill, 0)
            training_hours_needed[event.person][event.skill] += hours
            training_xp_spent[event.person][event.skill] += xp

        elif isinstance(event, Overlap):
            print(f'{now}: {event.person} has stunted {event.skills} together, with a {event.overlap} bonus.')
            # Remove any existing overlap on these skills.
            person = people[event.person]
            person.overlaps = [o for o in person.overlaps if o.skills != event.skills]
            person.overlaps.append(event)

        elif isinstance(event, Simulate):
            # Let's get to work.
            if now is None:
                raise ValueError('No start date specified.')
            while event.until is None or now <= event.until:
                for person in people.values():
                    # First, compute the maximum training we can apply to each skill
                    # that this person is training. This is the smaller of 8 hours and
                    # whatever limits may apply.
                    training_desired = {}
                    trained_hours = {}
                    for training in person.training:
                        training_desired[training.skill] = 8
                        trained_hours[training.skill] = 0
                    for limit in person.limits:
                        if limit.skill in training_desired:
                            training_desired[limit.skill] = min(training_hours[limit.skill], limit.hours_per_day)
                    # We also need to consider overlap. If two skills are trained simultaneously,
                    # we try to balance the training time between them. Overlap bonuses are calculated last,
                    # based on total training time for the day -- strictly speaking this means you can get
                    # overlap for skills that weren't technically trained at the same time, but it's much
                    # simpler to implement.
                    #
                    # We calculate bonus time first, since it only applies to a subset of the skills.
                    # (And those skills can have limits to total training time that don't apply to the others, i.e. Integrity.)
                    for bonus in person.bonuses:
                        skills = set(bonus.skills).intersection(training_desired.keys())
                        total_time = bonus.hours_per_day
                        # The loop below balances training time as evenly as possible, without wasting time.
                        while skills and total_time > 0:
                            time_per_skill = total_time / len(skills)
                            for skill in skills:
                                trained_time = min(training_desired[skill], time_per_skill)
                                trained_hours[skill] += trained_time
                                training_desired[skill] -= trained_time
                                total_time -= trained_time
                                if training_desired[skill] <= 0:
                                    skills.remove(skill)
                                    training_desired.pop(skill)
                    # Remaining (base) time is handled the same way, except there's no limits on what skills can be trained.
                    total_time = person.schedule.hours_per_day
                    while training_desired and total_time > 0:
                        time_per_skill = total_time / len(training_desired)
                        for skill in training_desired:
                            trained_time = min(training_desired[skill], time_per_skill)
                            trained_hours[skill] += trained_time
                            training_desired[skill] -= trained_time
                            total_time -= trained_time
                            if training_desired[skill] <= 0:
                                training_desired.pop(skill)
                    # Finally, apply overlap bonuses.
                    overlap_bonus_applied = set()
                    for overlap in person.overlaps:
                        overlapping_skills = set(overlap.skills).intersection(trained_hours.keys())
                        if len(overlapping_skills) > 1:
                            for skill in overlapping_skills:
                                assert skill not in overlap_bonus_applied, f'Overlap bonus applied twice to {skill}. Baughn made this way too complicated. What does this even mean, conceptually?'
                                trained_hours[skill] *= 1 + overlap.overlap
                                overlap_bonus_applied.add(skill)
                    # Phew. Training time is now calculated. Let's add it to the running tally.
                    for skill, hours in trained_hours.items():
                        training_hours_applied[person.name][skill] += hours
                    # Check if any skills have been completed.
                    for training in person.training:
                        if training_hours_applied[person.name][training.skill] >= training_hours_needed[person.name][training.skill]:
                            person.skills[training.skill] = training.target
                            print(f'{now}: {person.name} has completed training in {training.skill} to {training.target}.')
                            # Remove the training from the list.
                            person.training = [t for t in person.training if t.skill != training.skill]
                            # Simplify the running tallies.
                            training_hours_applied[person.name].pop(training.skill)
                            training_hours_needed[person.name].pop(training.skill)
                            training_xp_spent[person.name].pop(training.skill)
                # Simple! Now advance the date.
                now += datetime.timedelta(days=1)
                # Well, at least one thing is simple.
                # Check if we're done.
                for person in people.values():
                    if person.training:
                        break
                else:
                    print(f'{now}: All training completed.')
                    break
        else:
            raise ValueError(f'Unknown event type: {event}')


if __name__ == '__main__':
    simulate()
