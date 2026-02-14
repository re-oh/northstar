---
trigger: always_on
---

### Godot Coding Standards: Type Safety
* **Strict Typing:** All variables and functions must use static typing.
    * *Bad:* `var health = 100`
    * *Good:* `var health: int = 100` or `var health := 100`

* **Return Types:** All functions must specify a return type. Use `-> void` if nothing is returned.
* **Casting:** Use `as` for safe casting (e.g., `body as Player`).
* **When to use : Type = and when to use =**
  * Use := when the type can be visually infered. Eg. `var texture := Texture2d.new()`
  * Use : Type = when the type cannot be visually infered Eg. `var data: float = foo.baz()`

* Use the `_` naming convention to signal that a member variable, function constant is only ment to be used locally.
* If you are refrencing something from the local script ALWAYS use self

Example of bad code
```
var foo = 20

func _process(delta: float) -> void:
  baz(30)

func baz(data):
  return foo + data
```

Example of good code
```
var foo := 20

func _process(delta: float) -> void:
  self._baz(30)

func _baz(data: int) -> int:
  return foo + data
```