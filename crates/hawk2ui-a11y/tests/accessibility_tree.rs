use hawk2ui_a11y::{A11yAction, A11yBounds, A11yNode, A11yRole, A11yTree, CheckedState};
use serde::{Serialize, de::DeserializeOwned};

fn assert_serde<T: Serialize + DeserializeOwned>() {}

#[test]
fn tree_records_preserve_shape_identity_bounds_and_hierarchy() {
    let tree = A11yTree::new(
        A11yNode::new("root", A11yRole::Window)
            .name("Main Window")
            .bounds(A11yBounds::new(0.0, 0.0, 800.0, 600.0))
            .child(
                A11yNode::new("gain", A11yRole::Slider)
                    .name("Gain")
                    .description("Output gain")
                    .value("-6 dB")
                    .focused(true)
                    .bounds(A11yBounds::new(20.0, 20.0, 120.0, 32.0))
                    .action(A11yAction::Increment)
                    .action(A11yAction::Decrement),
            )
            .child(
                A11yNode::new("enabled", A11yRole::Checkbox)
                    .name("Enabled")
                    .checked(CheckedState::Checked)
                    .disabled(false),
            ),
    );

    assert_eq!(tree.root.id, "root");
    assert_eq!(tree.root.children[0].id, "gain");
    assert_eq!(tree.root.children[0].bounds.unwrap().width, 120.0);
    assert_eq!(
        tree.find("enabled").unwrap().checked,
        Some(CheckedState::Checked)
    );
}

#[test]
fn tree_records_are_serializable_contracts() {
    assert_serde::<A11yTree>();
    assert_serde::<A11yNode>();
    assert_serde::<A11yBounds>();
    assert_serde::<A11yAction>();
    assert_serde::<A11yRole>();
}
