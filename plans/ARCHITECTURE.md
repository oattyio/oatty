
implement Component architecture for remaining screens/widgets using crates/tui/src/component.rs as the trait

Root (app-wide): use The Elm Architecture (TEA) — a single Model, update(msg), and view(model) loop to orchestrate global state, route messages, and control mode switches (Guided vs Power Mode, preview vs live workflow runs, suspend/handoff for shells). TEA’s message-driven flow keeps complex behavior predictable and testable. 
Ratatui

Screens & widgets: implement each major area (Search/List, Inputs, Table, Workflow Steps, Logs, Hint Bar, etc.) as Components that encapsulate their own state + init / handle_events / update / render. This keeps concerns local, avoids a giant monolith update, and maps well to our modular feature set (tables, fuzzy input, steps view). The Ratatui docs explicitly outline a trait-style component pattern and even provide a template. 
Ratatui

This hybrid is directly aligned with Ratatui’s three documented options (TEA, Component, Flux) and leverages the strengths of the first two. 
Ratatui

Why this fits the CLI/TUI

Complex but cohesive: Workflows, async calls, plugin invocations, and shell handoff are coordinated cleanly via root TEA messages; each view stays small and focused as a component.

Testability: Root update is pure and easy to unit-test; components can be snapshotted and tested independently.

Extensibility: New plugin-driven panels or MCP-powered tools can arrive as new Components without touching the core loop.

Performance: Components only update what they own; TEA provides a simple, predictable render path.

//////////////////////
The Elm Architecture (TEA)
6 min. readView original
When building terminal user interfaces (TUI) with ratatui, it’s helpful to have a solid structure for organizing your application. One proven architecture comes from the Elm language, known simply as The Elm Architecture (TEA).

In this section, we’ll explore how to apply The Elm Architecture principles to ratatui TUI apps.

The Elm Architecture: A Quick Overview
At its core, TEA is split into three main components:

Model: This is your application’s state. It contains all the data your application works with.
Update: When there’s a change (like user input), the update function takes the current model and the input, and produces a new model.
View: This function is responsible for displaying your model to the user. In Elm, it produces HTML. In our case, it’ll produce terminal UI elements.
Following TEA principles typically involves ensuring that you do the following things:

Define Your Model
Handling Updates
Rendering the View
1. Define Your Model
In ratatui, you’ll typically use a struct to represent your model:

struct Model {

//... your application's data goes here

}

For a counter app, our model may look like this:

#[derive(Debug, Default)]

struct Model {

counter: i32,

running_state: RunningState,

}

#[derive(Debug, Default, PartialEq, Eq)]

enum RunningState {

#[default]

Running,

Done,

}

2. Handling Updates
Updates in TEA are actions triggered by events, such as user inputs. The core idea is to map each of these actions or events to a message. This can be achieved by creating an enum to keep track of messages. Based on the received message, the current state of the model is used to determine the next state.

Defining a Message enum

enum Message {

//... various inputs or actions that your app cares about

// e.g., ButtonPressed, TextEntered, etc.

}

For a counter app, our Message enum may look like this:

#[derive(PartialEq)]

enum Message {

Increment,

Decrement,

Reset,

Quit,

}

update() function

The update function is at the heart of this process. It takes the current model and a message, and decides how the model should change in response to that message.

A key feature of TEA is immutability. Hence, the update function should avoid direct mutation of the model. Instead, it should produce a new instance of the model reflecting the desired changes.

fnupdate(model:&Model, msg: Message) -> Model {

matchmsg {

// Match each possible message and decide how the model should change

// Return a new model reflecting those changes

}

}

In TEA, it’s crucial to maintain a clear separation between the data (model) and the logic that alters it (update). This immutability principle ensures predictability and makes the application easier to reason about.

In TEA, the update() function can not only modify the model based on the Message, but it can also return another Message. This design can be particularly useful if you want to chain messages or have an update lead to another update.

For example, this is what the update() function may look like for a counter app:

fnupdate(model:&mut Model, msg: Message) -> Option<Message> {

matchmsg {

Message::Increment => {

model.counter +=1;

ifmodel.counter >50 {

return Some(Message::Reset);

}

}

Message::Decrement => {

model.counter -=1;

ifmodel.counter < -50 {

return Some(Message::Reset);

}

}

Message::Reset =>model.counter =0,

Message::Quit => {

// You can handle cleanup and exit here

model.running_state = RunningState::Done;

}

};

None

}

Remember that this design choice means that the main loop will need to handle the returned message, calling update() again based on that returned message.

Returning a Message from the update() function allows a developer to reason about their code as a “Finite State Machine”. Finite State Machines operate on defined states and transitions, where an initial state and an event (in our case, a Message) lead to a subsequent state. This cascading approach ensures that the system remains in a consistent and predictable state after handling a series of interconnected events.

Here’s a state transition diagram of the counter example from above:

While TEA doesn’t use the Finite State Machine terminology or strictly enforce that paradigm, thinking of your application’s state as a state machine can allow developers to break down intricate state transitions into smaller, more manageable steps. This can make designing the application’s logic clearer and improve code maintainability.

3. Rendering the View
The view function in the Elm Architecture is tasked with taking the current model and producing a visual representation for the user. In the case of ratatui, it translates the model into terminal UI elements. It’s essential that the view function remains a pure function: for a given state of the model, it should always produce the same UI representation.

fnview(model:&Model) {

//... use `ratatui` functions to draw your UI based on the model's state

}

Every time the model is updated, the view function should be capable of reflecting those changes accurately in the terminal UI.

A view for a simple counter app might look like:

fnview(model:&mut Model, frame:&mut Frame) {

frame.render_widget(

Paragraph::new(format!("Counter: {}", model.counter)),

frame.area(),

);

}

In TEA, you are expected to ensure that your view function is side-effect free. The view() function shouldn’t modify global state or perform any other actions. Its sole job is to map the model to a visual representation.

For a given state of the model, the view function should always produce the same visual output. This predictability makes your TUI application easier to reason about and debug.

In ratatui, there are StatefulWidgets which require a mutable reference to state during render.

For this reason, you may choose to forego the view immutability principle. For example, if you were interested in rendering a List, your view function may look like this:

fnview(model:&mut Model, f:&mut Frame) {

letitems=model.items.items.iter().map(|element| ListItem::new(element)).collect();

f.render_stateful_widget(List::new(items), f.area(), &mutmodel.items.state);

}

fnmain() {

loop {

...

terminal.draw(|f|view(&mutmodel, f) )?;

...

}

}

Another advantage of having access to the Frame in the view() function is that you have access to setting the cursor position, which is useful for displaying text fields. For example, if you wanted to draw an input field using tui-input, you might have a view that looks like this:

fnview(model:&mut Model, f:&mut Frame) {

letarea=f.area();

letinput= Paragraph::new(app.input.value());

f.render_widget(input, area);

ifmodel.mode == Mode::Insert {

f.set_cursor(

(area.x +1+self.input.cursor() as u16).min(area.x +area.width -2),

area.y +1

)

}

}

Putting it all together
When you put it all together, your main application loop might look something like:

Listen for user input.
Map input to a Message
Pass that message to the update function.
Draw the UI with the view function.
This cycle repeats, ensuring your TUI is always up-to-date with user interactions.

As an illustrative example, here’s the Counter App refactored using TEA.

The notable difference from before is that we have a Model struct that captures the app state, and a Message enum that captures the various actions your app can take.

use std::time::Duration;

use ratatui::{

crossterm::event::{self, Event, KeyCode},

widgets::Paragraph,

Frame,

};

#[derive(Debug, Default)]

struct Model {

counter: i32,

running_state: RunningState,

}

#[derive(Debug, Default, PartialEq, Eq)]

enum RunningState {

#[default]

Running,

Done,

}

#[derive(PartialEq)]

enum Message {

Increment,

Decrement,

Reset,

Quit,

}

fnmain() -> color_eyre::Result<()> {

tui::install_panic_hook();

letmutterminal= tui::init_terminal()?;

letmutmodel= Model::default();

whilemodel.running_state != RunningState::Done {

// Render the current view

terminal.draw(|f|view(&mutmodel, f))?;

// Handle events and map to a Message

letmutcurrent_msg=handle_event(&model)?;

// Process updates as long as they return a non-None message

whilecurrent_msg.is_some() {

current_msg=update(&mutmodel, current_msg.unwrap());

}

}

tui::restore_terminal()?;

Ok(())

}

fnview(model:&mut Model, frame:&mut Frame) {

frame.render_widget(

Paragraph::new(format!("Counter: {}", model.counter)),

frame.area(),

);

}

/// Convert Event to Message

///

/// We don't need to pass in a `model` to this function in this example

/// but you might need it as your project evolves

fnhandle_event(_:&Model) -> color_eyre::Result<Option<Message>> {

if event::poll(Duration::from_millis(250))? {

iflet Event::Key(key) = event::read()? {

ifkey.kind == event::KeyEventKind::Press {

return Ok(handle_key(key));

}

}

}

Ok(None)

}

fnhandle_key(key: event::KeyEvent) -> Option<Message> {

matchkey.code {

KeyCode::Char('j') => Some(Message::Increment),

KeyCode::Char('k') => Some(Message::Decrement),

KeyCode::Char('q') => Some(Message::Quit),

_=> None,

}

}

fnupdate(model:&mut Model, msg: Message) -> Option<Message> {

matchmsg {

Message::Increment => {

model.counter +=1;

ifmodel.counter >50 {

return Some(Message::Reset);

}

}

Message::Decrement => {

model.counter -=1;

ifmodel.counter < -50 {

return Some(Message::Reset);

}

}

Message::Reset =>model.counter =0,

Message::Quit => {

// You can handle cleanup and exit here

model.running_state = RunningState::Done;

}

};

None

}

mod tui {

use ratatui::{

backend::{Backend, CrosstermBackend},

crossterm::{

terminal::{

disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,

},

ExecutableCommand,

},

Terminal,

};

use std::{io::stdout, panic};

pubfninit_terminal() -> color_eyre::Result<Terminal<impl Backend>> {

enable_raw_mode()?;

stdout().execute(EnterAlternateScreen)?;

letterminal= Terminal::new(CrosstermBackend::new(stdout()))?;

Ok(terminal)

}

pubfnrestore_terminal() -> color_eyre::Result<()> {

stdout().execute(LeaveAlternateScreen)?;

disable_raw_mode()?;

Ok(())

}

pubfninstall_panic_hook() {

letoriginal_hook= panic::take_hook();

panic::set_hook(Box::new(move|panic_info| {

stdout().execute(LeaveAlternateScreen).unwrap();

disable_raw_mode().unwrap();

original_hook(panic_info);

}));

}

}

Component Architecture
If you are interested in a more object oriented approach to organizing TUIs, you can use a Component based approach.

A couple of projects in the wild use this approach

https://github.com/TaKO8Ki/gobang
https://github.com/nomadiz/edma
We also have a component template that has an example of this Component based approach:

https://github.com/ratatui/templates/tree/main/component
We already covered TEA in the previous section. The Component architecture takes a slightly more object oriented trait based approach.

Each component encapsulates its own state, event handlers, and rendering logic.

Component Initialization (init) - This is where a component can set up any initial state or resources it needs. It’s a separate process from handling events or rendering.

Event Handling (handle_events, handle_key_events, handle_mouse_events) - Each component has its own event handlers. This allows for a finer-grained approach to event handling, with each component only dealing with the events it’s interested in. This contrasts with Elm’s single update function that handles messages for the entire application.

State Update (update) - Components can have their own local state and can update it in response to actions. This state is private to the component, which differs from Elm’s global model.

Rendering (render) - Each component defines its own rendering logic. It knows how to draw itself, given a rendering context. This is similar to Elm’s view function but on a component-by-component basis.

Here’s an example of the Component trait implementation you might use:

use color_eyre::eyre::Result;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;

use crate::{action::Action, event::Event, terminal::Frame};

pub trait Component {
  fn init(&mut self) -> Result<()> {
    Ok(())
  }
  fn handle_events(&mut self, event: Option<Event>) -> Action {
    match event {
      Some(Event::Quit) => Action::Quit,
      Some(Event::Tick) => Action::Tick,
      Some(Event::Key(key_event)) => self.handle_key_events(key_event),
      Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event),
      Some(Event::Resize(x, y)) => Action::Resize(x, y),
      Some(_) => Action::Noop,
      None => Action::Noop,
    }
  }
  fn handle_key_events(&mut self, key: KeyEvent) -> Action {
    Action::Noop
  }
  fn handle_mouse_events(&mut self, mouse: MouseEvent) -> Action {
    Action::Noop
  }
  fn update(&mut self, action: Action) -> Action {
    Action::Noop
  }
  fn render(&mut self, f: &mut Frame, rect: Rect);
}

One advantage of this approach is that it incentivizes co-locating the handle_events, update and render functions on a component level.