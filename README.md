# Peachy 🍑

Peachy is a simple async task manager, similar to thread pool but cooler and for structs

It is build using `tokio` and does not support `async-std` and std threads

Example:
```rust
use peachy::prelude::*;

#[tokio::main]
async fn main() {
    Manager::new()
        .add_routine(HelloWorldRoutine)
        .run()
        .await
        .unwrap();
}

struct HelloWorldRoutine;

impl Routine for HelloWorldRoutine {
    async fn run(self) -> ManagerResult {
        println!("Hello, World!");
        Ok(())
    }
}
```

This will output `Hello, World!`

It is also provided with Mediator, which allows routines to communicate to each other via messages, and the Mediator itself is also a routine

Example:
```rust
use peachy::prelude::*;
use peachy::mediator::*;

#[tokio::main]
async fn main() -> ManagerResult {
    let mediator = Mediator::new();

    Manager::new()
        .add_routine(SenderRoutine { con: mediator.connect(AppRoute::SenderRoutine).await })
        .add_routine(ReceiverRoutine { con: mediator.connect(AppRoute::ReceiverRoutine).await })
        .add_routine(mediator) // should be added after all connections
        .run()
        .await?;

    Ok(())
}

struct SenderRoutine {
    con: Connector<AppRoute, AppEvent>
}

impl Routine for SenderRoutine {
    async fn run(mut self) -> ManagerResult {
        self.con.send(AppRoute::ReceiverRoutine, AppEvent::ReceiverRoutine(ReceiverRoutineEvent::Print("Hello, World!".to_string()))).await?;
        Ok(())
    }
}

struct ReceiverRoutine {
    con: Connector<AppRoute, AppEvent>
}

impl Routine for ReceiverRoutine {
    async fn run(mut self) -> ManagerResult {
        if let AppEvent::ReceiverRoutine(ReceiverRoutineEvent::Print(text)) = self.con.recv().await.unwrap() {
            println!("{}", text);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AppRoute {
    SenderRoutine,
    ReceiverRoutine,
}

#[derive(Clone, PartialEq)]
enum AppEvent {
    ReceiverRoutine(ReceiverRoutineEvent)
}

#[derive(Clone, PartialEq)]
enum ReceiverRoutineEvent {
    Print(String)
}
```

The output of this program will also be `Hello, World!`
