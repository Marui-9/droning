use std::{
    iter,
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::Sender;
use rand::Rng;
use wg_2024::network::NodeId;

use crate::{
    application::turn_handler::TurnHandlerArc,
    client::utils::{input_with_check, wait_for_input},
};

use super::{
    base_client::ClientBehaviour,
    card::{self, Card},
};

pub struct ClientGame<B>
where
    B: ClientBehaviour,
{
    id: NodeId,
    hand: Vec<Card<B>>,
    cards_played: usize,
    card_sender: Sender<Card<B>>,
    turn_handler: TurnHandlerArc,
}

const STARTING_HAND_SIZE: usize = 5;

impl<B> ClientGame<B>
where
    B: ClientBehaviour,
{
    fn draw_card() -> Card<B> {
        let mut rng = rand::thread_rng();

        let mut all_cards = card::generic_cards().into_iter().chain(B::cards());

        let max_prob = all_cards.clone().map(|card| card.prob_value()).sum();
        let mut pick = rng.gen_range(0..max_prob);

        loop {
            let next_card = all_cards.next().unwrap();
            if pick < next_card.prob_value() {
                break next_card;
            }
            pick -= next_card.prob_value();
        }
    }

    fn starting_hand() -> Vec<Card<B>> {
        iter::once(card::flood_request_card())
            .chain(iter::once(card::the_navigator_card()))
            .chain(iter::repeat_with(|| Self::draw_card()))
            .take(STARTING_HAND_SIZE)
            .collect()
    }

    pub fn new(id: NodeId, card_sender: Sender<Card<B>>, turn_handler: TurnHandlerArc) -> Self {
        Self {
            id,
            hand: Self::starting_hand(),
            cards_played: 0,
            card_sender,
            turn_handler,
        }
    }

    pub fn start_thread(
        id: NodeId,
        card_sender: Sender<Card<B>>,
        turn_handler: TurnHandlerArc,
    ) -> JoinHandle<()>
    where
        B: 'static,
    {
        thread::spawn(move || Self::new(id, card_sender, turn_handler).run())
    }

    fn subscribe_to_turn_handler(&self) {
        self.turn_handler.lock().unwrap().subscribe(self.id);
    }

    fn is_my_turn(&self) -> bool {
        let current_turn = self.turn_handler.lock().unwrap().current_turn();
        current_turn == self.id
    }

    pub fn yield_turn(&self) {
        self.turn_handler.lock().unwrap().yield_turn();
    }

    fn unsubscribe_from_turn_handler(&self) {
        self.turn_handler.lock().unwrap().unsubscribe(self.id);
    }

    fn print_hand(&self) {
        for (i, card) in self.hand.iter().enumerate() {
            println!();
            card.print_card(i + 1);
        }
    }

    pub(crate) fn draw_new_card(&mut self) {
        let new_card = Self::draw_card();
        self.hand.push(new_card);
    }

    fn handle_turn(&mut self) -> bool {
        println!("It's Client {}'s turn", self.id);

        self.draw_new_card();

        self.hand.insert(0, card::yield_turn_card());

        self.cards_played = 0;

        loop {
            println!("Client {}'s hand:", self.id);

            self.print_hand();

            let choice: usize = input_with_check("Choose your card: ".to_string(), |&choice| {
                choice <= self.hand.len()
            });

            self.cards_played += 1;

            println!("You played the following card:");

            let card = self.hand.remove(choice - 1);

            card.print_card(choice);

            thread::sleep(Duration::from_millis(500));

            let is_yield = card.is_yield_turn();
            let is_forget_topology = card.is_forget_topology();

            self.card_sender
                .send(card.clone())
                .expect("unable to send card");
            if self.card_sender.send(card).is_err() {
                return true;
            }

            if is_yield {
                if self.cards_played == 1 {
                    println!("Since you didn't play any card, you will draw a new card");
                    self.draw_new_card();
                }
                break;
            }

            if is_forget_topology {
                self.hand.push(card::flood_request_card());
            }

            wait_for_input();
        }

        self.yield_turn();

        false
    }

    pub fn run(&mut self) {
        self.subscribe_to_turn_handler();

        thread::sleep(Duration::from_millis(500));

        loop {
            if self.is_my_turn() {
                if self.handle_turn() {
                    break;
                }
            } else {
                thread::sleep(Duration::from_millis(500));
            }
        }

        self.unsubscribe_from_turn_handler();
    }
}
