mod app;
mod rows;

use app::AppModel;
use relm4::RelmApp;

fn main() {
    dotenvy::dotenv().ok();
    let app = RelmApp::new("io.github.MCR_Project.gtasks");
    relm4::set_global_css(
        "
        .task-completed {
            text-decoration: line-through;
            opacity: 0.5;
            transition: opacity 300ms ease-in-out;
        }
        .task-check {
            min-width: 44px;
            min-height: 44px;
        }
        ",
    );
    app.run::<AppModel>(());
}

#[cfg(test)]
mod tests {
    use gtasks_core::parse_nlp_task;

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
