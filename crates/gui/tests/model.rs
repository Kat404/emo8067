//! The window shows the model: the panels are the model's own, and the rows
//! are the panels' own.
//!
//! The rule the crate was made under is that the machine is read in one
//! place — `Panels::of` — and that the window draws what the model gives.
//!
//! This is what that looks like from outside the crate. It is not a search
//! of the crate's own text: a search cannot tell a second reading of the
//! machine from the first one, and it refuses the code the crate itself
//! writes. The shape says it instead — the drawing takes the model, and
//! the constructor is the only place that takes the machine.

use emo8067_debug::panels::Panels;
use emo8067_gui::app::App;
use emo8067_gui::ui::{Here, listing};

#[test]
fn the_app_draws_the_model_and_not_a_second_reading() {
    let app = App::new(None).expect("the machine is empty");

    assert_eq!(
        app.panels,
        Panels::of(&app.cpu, &app.ui),
        "the panels are the model's own, and the model read the machine"
    );
    assert_eq!(
        app.rows,
        listing(
            &app.panels,
            Here {
                segment: 0,
                offset: 0
            }
        ),
        "and the rows are the panels' own"
    );
}
