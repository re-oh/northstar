---
trigger: always_on
---

### Godot Coding Standards: Signals
* **Syntax:** Use the Godot 4.x `Callable` syntax for connections.
    * *Correct:* `button.pressed.connect(_on_button_pressed)`
    * *Incorrect:* `button.connect("pressed", self, "_on_button_pressed")`
* **Custom Signals:** When defining signals, strictly type their arguments if possible: `signal health_changed(new_value: int)`