//! A question over the window before an act that is hard to take back: its title, its text, the
//! label of the yes and the act a yes runs, and a second answer beside it when there is one (Don't
//! save beside Save).

pub struct Question<T> {
    pub title: String,
    pub text: String,
    pub yes: String,
    pub act: T,
    /// A second answer, between the yes and Cancel: its label and its act.
    pub alt: Option<(String, T)>,
}

impl<T> Question<T> {
    pub fn new(title: &str, text: impl Into<String>, yes: &str, act: impl Into<T>) -> Self {
        Self {
            title: title.to_owned(),
            text: text.into(),
            yes: yes.to_owned(),
            act: act.into(),
            alt: None,
        }
    }

    /// The question with a second answer.
    pub fn or(mut self, label: &str, act: impl Into<T>) -> Self {
        self.alt = Some((label.to_owned(), act.into()));
        self
    }
}
