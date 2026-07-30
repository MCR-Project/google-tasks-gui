use chrono::Datelike;
use std::collections::HashMap;
use crate::api::TaskLocal;

/// Parses a task title using basic NLP rules to extract natural language due dates
/// like "today", "tomorrow", "next week", "next monday", etc.
pub fn parse_nlp_task(input: &str) -> (String, Option<chrono::DateTime<chrono::Utc>>) {
    let lower = input.to_lowercase();
    let today = chrono::Local::now().date_naive();
    let mut due_date: Option<chrono::NaiveDate> = None;
    let mut matched_phrase: Option<&str> = None;

    if lower.contains("next week") {
        due_date = Some(today + chrono::Duration::days(7));
        matched_phrase = Some("next week");
    } else if lower.contains("tomorrow") {
        due_date = Some(today + chrono::Duration::days(1));
        matched_phrase = Some("tomorrow");
    } else if lower.contains("today") {
        due_date = Some(today);
        matched_phrase = Some("today");
    } else {
        let patterns = [
            ("next monday", chrono::Weekday::Mon),
            ("next tuesday", chrono::Weekday::Tue),
            ("next wednesday", chrono::Weekday::Wed),
            ("next thursday", chrono::Weekday::Thu),
            ("next friday", chrono::Weekday::Fri),
            ("next saturday", chrono::Weekday::Sat),
            ("next sunday", chrono::Weekday::Sun),
        ];
        for (phrase, weekday) in patterns {
            if lower.contains(phrase) {
                let mut d = today + chrono::Duration::days(1);
                while d.weekday() != weekday {
                    d += chrono::Duration::days(1);
                }
                due_date = Some(d);
                matched_phrase = Some(phrase);
                break;
            }
        }

        if due_date.is_none() {
            let single_weekdays = [
                ("monday", chrono::Weekday::Mon),
                ("tuesday", chrono::Weekday::Tue),
                ("wednesday", chrono::Weekday::Wed),
                ("thursday", chrono::Weekday::Thu),
                ("friday", chrono::Weekday::Fri),
                ("saturday", chrono::Weekday::Sat),
                ("sunday", chrono::Weekday::Sun),
            ];
            for (name, weekday) in single_weekdays {
                let words: Vec<&str> = lower.split_whitespace().collect();
                if words.contains(&name) {
                    let mut d = today + chrono::Duration::days(1);
                    while d.weekday() != weekday {
                        d += chrono::Duration::days(1);
                    }
                    due_date = Some(d);
                    matched_phrase = Some(name);
                    break;
                }
            }
        }
    }

    let clean_title = if let Some(phrase) = matched_phrase {
        let mut result = String::new();
        let mut remaining = input;
        while let Some(pos) = remaining.to_lowercase().find(phrase) {
            result.push_str(&remaining[..pos]);
            remaining = &remaining[pos + phrase.len()..];
        }
        result.push_str(remaining);
        result
    } else {
        input.to_string()
    };

    let clean_trimmed = clean_title.trim().to_string();
    let final_title = if clean_trimmed.is_empty() {
        input.trim().to_string()
    } else {
        clean_trimmed
    };

    let due_dt = due_date.map(|d| {
        let naive_dt = d.and_hms_opt(0, 0, 0).expect("Midnight is always valid");
        chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive_dt, chrono::Utc)
    });

    (final_title, due_dt)
}

/// Orders tasks hierarchically so subtasks follow their parent task
pub fn order_tasks_hierarchically(tasks: Vec<TaskLocal>) -> Vec<TaskLocal> {
    let mut parent_map: HashMap<Option<String>, Vec<TaskLocal>> = HashMap::new();

    for task in tasks {
        parent_map.entry(task.parent.clone()).or_default().push(task);
    }

    let mut ordered = Vec::new();
    let root_tasks = parent_map.remove(&None).unwrap_or_default();

    fn add_with_children(
        task: TaskLocal,
        parent_map: &mut HashMap<Option<String>, Vec<TaskLocal>>,
        ordered: &mut Vec<TaskLocal>,
    ) {
        let id = task.id.clone();
        ordered.push(task);
        if let Some(children) = parent_map.remove(&Some(id)) {
            for child in children {
                add_with_children(child, parent_map, ordered);
            }
        }
    }

    for root in root_tasks {
        add_with_children(root, &mut parent_map, &mut ordered);
    }

    for (_parent, orphans) in parent_map {
        for orphan in orphans {
            ordered.push(orphan);
        }
    }

    ordered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nlp_task_today_tomorrow() {
        let (title1, due1) = parse_nlp_task("Buy milk today");
        assert_eq!(title1, "Buy milk");
        assert!(due1.is_some());

        let (title2, due2) = parse_nlp_task("Finish report tomorrow");
        assert_eq!(title2, "Finish report");
        assert!(due2.is_some());
    }

    #[test]
    fn test_parse_nlp_task_weekday() {
        let (title, due) = parse_nlp_task("Team sync next monday");
        assert_eq!(title, "Team sync");
        assert!(due.is_some());
    }

    #[test]
    fn test_parse_nlp_task_no_date() {
        let (title, due) = parse_nlp_task("Read documentation");
        assert_eq!(title, "Read documentation");
        assert!(due.is_none());
    }
}
