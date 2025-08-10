use std::{thread::sleep, time::Duration};

use bevy::prelude::*;

#[derive(Component)]
struct Person;

#[derive(Component)]
struct Person2;

#[derive(Component)]
struct Name(String);

#[derive(Component)]
struct Name2(String);

fn add_people(mut commands: Commands) {
    commands.spawn((Person, Name("a".to_string())));

    commands.spawn((Person2, Name2("a2".to_string())));
}

fn greet_people(query: Query<&Name, With<Person>>) {
    for name in &query {
        for i in 0..10 {
            println!("hello {}!", name.0);
            sleep(Duration::from_millis(200));
        }
    }
}

fn greet_people2(query: Query<&Name2, With<Person2>>) {
    for name in &query {
        for i in 0..10 {
            println!("hello {}!", name.0);
            sleep(Duration::from_millis(2));
        }
    }
}

fn hello_world() {
    println!("hello world!");
}

fn main() {
    App::new()
        .add_systems(Startup, add_people)
        .add_systems(Update, (greet_people, hello_world, greet_people2))
        .run();
}
