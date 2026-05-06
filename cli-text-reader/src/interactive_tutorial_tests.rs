#[cfg(test)]
mod tests {
  use crate::interactive_tutorial::*;

  #[test]
  fn test_tutorial_steps_count() {
    let steps = get_interactive_tutorial_steps();
    assert_eq!(steps.len(), 9, "Should have exactly 9 tutorial steps");
  }

  #[test]
  fn test_tutorial_steps_have_titles() {
    let steps = get_interactive_tutorial_steps();
    for (i, step) in steps.iter().enumerate() {
      assert!(
        !step.title.is_empty(),
        "Step {} should have a non-empty title",
        i + 1
      );
    }
  }
}
