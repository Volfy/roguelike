/////// TODO: CONTINUE TUTORIAL part 3
/// figure out how to implement walls as just Object? -- maybe in another version
/// research tcod / libtcod
/// choose nicer font

// imports
use tcod::colors::*;
use tcod::console::*;
use tcod::map::{FovAlgorithm, Map as FovMap};
use std::cmp;
use rand::Rng;

// size of window
const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

// size of the map
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 43;

// sizes for GUI
const BAR_WIDTH: i32 = 20;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;

// message pos size
const MSG_X: i32 = BAR_WIDTH + 2;
const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;

// size of rooms for dungeon generator
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 17;

// max monsters per rm
const MAX_ROOM_MONSTERS: i32 = 3;

// colors of map elements
const COLOR_DARK_WALL: Color = Color {r: 0, g: 50, b:50};
const COLOR_LIGHT_WALL: Color = Color {r: 70, g: 100, b:80};

const COLOR_DARK_GROUND: Color = Color {r: 10, g: 20, b: 25};
const COLOR_LIGHT_GROUND: Color = Color {r: 170, g: 140, b: 25};


// sets Field of View details
const FOV_ALGO: FovAlgorithm = FovAlgorithm::Shadow;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

// frames per second 
const LIMIT_FPS: i32 = 20; 

// player first object
const PLAYER: usize = 0;






/// OBJECTS ///
////////////////////////////////////////////
////////////////////////////////////////////
///////////////

#[derive(Clone, Copy, Debug, PartialEq)]
enum Ai {
    Basic,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut Object, game: &mut Game) {
        use DeathCallback::*;
        let callback = match self {
            Player => player_death,
            Monster => monster_death,
        };
        callback(object, game);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}


// holds libtcod related values
struct Tcod {
  root: Root,
  con: Offscreen,
  panel: Offscreen,
  fov: FovMap,
}

// a tile of the map and its properties
#[derive(Clone, Copy, Debug)]
struct Tile {
    blocked: bool,
    explored: bool,
    block_sight: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            explored: false,
            block_sight: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            explored: false,
            block_sight: true,
        }
    }
}

struct Messages {
    messages: Vec<(String, Color)>,
}
impl Messages {
    pub fn new() -> Self {
        Self { messages: vec![]}
    }
    // add new msg as a tuple w/ txt and color
    pub fn add<T: Into<String>>(&mut self, message: T, color: Color) {
        self.messages.push((message.into(), color));
    }
    // create doubleendediterator over messages
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &(String, Color)> {
        self.messages.iter()
    }
}

// rectangle on map ie a room
#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    // constructor
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }
    // find center
    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        (center_x, center_y)
    }
    // returns true if rect intersects with another one
    pub fn intersects_with(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2)
        && (self.x2 >= other.x1)
        && (self.y1 <= other.y2)
        && (self.y2 >= other.y1)
    }
}

// combat related properties and methods (monster, player, npc)
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defense: i32,
    power: i32,
    on_death: DeathCallback,
}




// generic object, represented by a character on screen
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<Ai>,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, name: &str, color: Color,  blocks: bool) -> Self {
        Object { 
            x: x,
            y: y,
            char: char,
            color: color,
            name: name.into(),
            blocks: blocks,
            alive: false,
            fighter: None,
            ai: None,
        }
    }

    
    // set color and draw character representing object
    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    // returns distance to another object
    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
    }

    pub fn take_damage(&mut self, damage: i32, game: &mut Game) {
        // apply damage if possible
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }
        // check for death, call death function
        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self, game);
            }
        }
    }

    pub fn attack(&mut self, target: &mut Object, game: &mut Game) {
        // a simple formula for attack damage
        let damage = self.fighter.map_or(0, |f| f.power) - target.fighter.map_or(0, |f| f.defense);
        if damage > 0 {
            // make target take damage
            game.messages.add(
                format!("{} sues {} for ${} million in damages.",
                self.name, target.name, damage
                ),
                WHITE,
            );
            target.take_damage(damage, game);
        } else {
            game.messages.add(
                format!(
                "{} sues {} but it gets dismissed",
                self.name, target.name
                ),
                WHITE,
            );
        }
    }

    // getter position
    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    // setter position
    pub fn set_pos(&mut self, x: i32, y:i32){
        self.x = x;
        self.y = y;
    }
}

// defines the map
type Map = Vec<Vec<Tile>>;

struct Game {
    map: Map,
    messages: Messages,
}



/// FUNCTIONS ///
///////////////////////////////////////////
///////////////////////////////////////////
/////////////////

// goes thru tiles in rect and makes them passable
fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

// horizontal tunnel. min and max used in case x1 > x2
fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) +1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

// vertical tunnel
fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) +1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

// checks if blocked
fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    // test tile first
    if map[x as usize][y as usize].blocked {
        return true;
    }
    // now check for blocking objects
    objects
        .iter()
        .any(|object| object.blocks && object.pos() == (x, y))
}

// move by given amount if dest not blocked
fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y+ dy, map, objects) {
        objects[id].set_pos(x+dx, y+dy);
    }
    
}

// moves player or attacks monster
fn player_move_or_attack(dx: i32, dy: i32, game: &mut Game, objects: &mut [Object]){
    // coords player move/attack to
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    // try find attackable object there
    let target_id = objects.iter().position(|object| object.fighter.is_some() && object.pos() == (x, y));

    // attack if target found, move otherwise
    match target_id {
        Some(target_id) => {
            let (player, target) = mut_two(PLAYER, target_id, objects);
            player.attack(target, game);
        }
        None => {
            move_by(PLAYER, dx, dy, &game.map, objects);
        }
    }
}

fn player_death(player: &mut Object, game: &mut Game) {
    // game ended!
    game.messages.add("You died! Capitalism reigns supreme.", RED);

    player.char = '%';
    player.color = DARK_RED;
}

fn monster_death(monster: &mut Object, game: &mut Game) {
    // transform to corpse
    game.messages.add(format!("{} is dead, yet surely, will be replaced.", monster.name), ORANGE);
    monster.char = '%';
    monster.color = DARK_RED;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name)
}

fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [Object]) {
    // vector from this object to target and distance
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;
    let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

    // normalize to length 1 while preserving direction then round and
    // convert to integer so movement restricted to map grid
    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, dx, dy, map, objects);
}

/// mutably borrow 2 separate elements from given slice
/// will panic when indexes are equal or oob
fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first_index != second_index);
    let split_at_index = cmp::max(first_index, second_index);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }
}


fn ai_take_turn(monster_id: usize, tcod: &Tcod, game: &mut Game, objects: &mut [Object]) {
    // a basic monster takes its turn. if u can see it it can see u
    let (monster_x, monster_y) = objects[monster_id].pos();
    if tcod.fov.is_in_fov(monster_x, monster_y) {
        if objects[monster_id].distance_to(&objects[PLAYER]) >= 2.0 {
            // move towards player if far away
            let (player_x, player_y) = objects[PLAYER].pos();
            move_towards(monster_id, player_x, player_y, &game.map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |f| f.hp >0) {
            // close enough - attack if player is still alive
            let (monster, player) = mut_two(monster_id, PLAYER, objects);
            monster.attack(player, game);
        }
    }
}



// creates monsters!! 
fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    // chooses rand no. monsters
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_monsters{
        // choose rand loc. for monster
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        let mut monster = if rand::random::<f32>() < 0.8 {
            // 80% chance of getting bezos (orc)
            // create bezos
            let mut bezos = Object::new(x, y, 'b', "bezos", BLACK, true);
            bezos.fighter = Some(Fighter {
                max_hp: 10,
                hp: 10,
                defense: 0,
                power: 3,
                on_death: DeathCallback::Monster,
            });
            bezos.ai = Some(Ai::Basic);
            bezos
        } else {
            // 20% for trump (troll)
            let mut trump = Object::new(x, y, 'T', "trump", BLACK, true);
            trump.fighter = Some(Fighter {
                max_hp: 16,
                hp: 16,
                defense: 1,
                power: 4,
                on_death: DeathCallback::Monster,
            });
            trump.ai = Some(Ai::Basic);
            trump
        };
        if !is_blocked(x, y, map, objects) {
            monster.alive = true;
            objects.push(monster);
        }
    }
}

fn render_bar(
    panel: &mut Offscreen,
    x: i32,
    y: i32,
    total_width: i32,
    name: &str,
    value: i32,
    maximum: i32,
    bar_color: Color,
    back_color: Color,
) {
    // render a bar, first calculate width
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // render background
    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    // now render bar on top
    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
    }

    // some centered text with values
    panel.set_default_foreground(WHITE);
    panel.print_ex(
        x + total_width / 2,
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        &format!("{}: {} / {}", name, value, maximum),
    );
}

// fill map 
fn make_map(objects: &mut Vec<Object>) -> Map {
    // fills map with blocked tiles
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    // pillars for testing map
    /* map[30][22] = Tile::wall();
       map[50][22] = Tile::wall(); */

    // create two rooms for testing
    /* let room1 = Rect::new(20, 15, 10, 15);
       let room2 = Rect::new(50, 15, 10, 15);
       create_room(room1, &mut map);
       create_room(room2, &mut map);
       create_h_tunnel(25, 55, 23, &mut map);*/


    // generate rooms
    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // random width and height
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        // random position within map
        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);

        //run thru rooms and see if they intersect
        let failed = rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));

        if !failed {
            // no intersections for this room

            // paint it to tiles
            create_room(new_room, &mut map);

            // add some content (ie monsters) to room
            place_objects(new_room, &map, objects);

            // center coords of new room
            let (new_x, new_y) = new_room.center();

            // this is the first room, where player starts
            if rooms.is_empty() {
                objects[PLAYER].set_pos(new_x, new_y);
            } else {
                // for all rooms after the first
                // connect to previous room with a tunnel

                // center coords of prev room
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                //toss a coin (random boolean)
                if rand::random() {
                    // first move horizontally, then vertically
                    create_h_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    // first move vert then hor
                    create_v_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, new_x, new_y, &mut map);
                }
            }

            // append new room to list
            rooms.push(new_room);
        }
    }

    map
    
}

// draws all objects in list
fn render_all(tcod: &mut Tcod, game: &mut Game, objects: &[Object], fov_recompute: bool){
    let mut to_draw: Vec<_> = objects
        .iter()
        .filter(|o| tcod.fov.is_in_fov(o.x, o.y))
        .collect();
    // sort so nonblocking objs come first
    to_draw.sort_by(|o1, o2| { o1.blocks.cmp(&o2.blocks)});
    // draw objects in list
    for object in &to_draw {
            object.draw(&mut tcod.con);
    }

    // recomputes fov if needed (player move)
    if fov_recompute {
        let player = &objects[PLAYER];
        tcod.fov
            .compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);
    }

    // set bg color for tiles
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let visible = tcod.fov.is_in_fov(x, y);
            let wall = game.map[x as usize][y as usize].block_sight;


            /*if wall {
                tcod.con
                    .set_char_background(x, y, COLOR_DARK_WALL, BackgroundFlag::Set);
            } else {
                tcod.con
                    .set_char_background(x, y, COLOR_DARK_GROUND, BackgroundFlag::Set);
            }*/


            let color = match(visible, wall) {
                // outside fov
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                // inside fov
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };

            let explored = &mut game.map[x as usize][y as usize].explored;
            // since it's visible, count it as explored
            if visible {
                *explored = true;
            }
            // show explored tiles only
            if *explored {
                tcod.con
                    .set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }

        
    }



    // show players stats
    /*tcod.root.set_default_foreground(WHITE);
    if let Some(fighter) = objects[PLAYER].fighter {
        tcod.root.print_ex(
            1,
            SCREEN_HEIGHT - 2,
            BackgroundFlag::None,
            TextAlignment::Left,
            format!("HP: {}/{} ", fighter.hp, fighter.max_hp),
        );
    }*/ 

    // blit contents of con to the root console & present it
    blit (
        &tcod.con,
        (0, 0),
        (MAP_WIDTH, MAP_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0,
    );

    // prep render gui
    tcod.panel.set_default_background(BLACK);
    tcod.panel.clear();

    let mut y = MSG_HEIGHT as i32;
    for &(ref msg, color) in game.messages.iter().rev() {
        let msg_height = tcod.panel.get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
        y -= msg_height;
        if y < 0 {
            break;
        }
        tcod.panel.set_default_foreground(color);
        tcod.panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);
    }

    let hp = objects[PLAYER].fighter.map_or(0, |f| f.hp);
    let max_hp = objects[PLAYER].fighter.map_or(0, |f| f.max_hp);
    render_bar (
        &mut tcod.panel,
        1,
        1,
        BAR_WIDTH,
        "HP",
        hp,
        max_hp,
        LIGHT_RED,
        DARKER_RED,
    );


    blit(
        &tcod.panel,
        (0,0),
        (SCREEN_WIDTH,PANEL_HEIGHT),
        &mut tcod.root,
        (0, PANEL_Y),
        1.0,
        1.0
    );




}

// handle keyboard input
fn handle_keys(tcod: &mut Tcod, game: &mut Game, objects: &mut Vec<Object>) -> PlayerAction {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;
    use PlayerAction::*;

    // gets key
    let key = tcod.root.wait_for_keypress(true);

    let player_alive = objects[PLAYER].alive;

    // specifies values we're interested in and what to do with them
    match (key, key.text(), player_alive) {
        // set alt+enter: toggle fullscreen
        (Key { code: Enter, alt: true, .. }, _, _) => 
        {  
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
            DidntTakeTurn
        },
        
        // exit game
        (Key { code: Escape, .. }, _, _) => return Exit, 

        // movement keys
        (Key { code: Up, .. }, _, true) => {
            player_move_or_attack(0,-1, game, objects);
            TookTurn
        },
        (Key { code: Down, .. }, _, true) => {
            player_move_or_attack(0,1, game, objects);
            TookTurn
        },
        (Key { code: Left, .. }, _, true) => {
            player_move_or_attack(-1,0, game, objects);
            TookTurn
        },
        (Key { code: Right, .. }, _, true) => {
            player_move_or_attack(1,0, game, objects);
            TookTurn
        },

        _ => DidntTakeTurn,
    }
}




/// MAIN FUNCTION ///
///////////////////////////////////////////////
///////////////////////////////////////////////
/////////////////////




fn main() {
    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("yet another roguelike v 0.01")
        .init();

    let mut tcod = Tcod { 
        root, 
        con: Offscreen::new(MAP_WIDTH, MAP_HEIGHT),
        panel: Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT),
        fov: FovMap::new(MAP_WIDTH, MAP_HEIGHT),
    };
    
    // create object representing the player
    let mut player = Object::new(0, 0, '@', "player", WHITE, true);
    player.alive = true;
    player.fighter = Some(Fighter {
        max_hp: 30,
        hp: 30,
        defense: 2,
        power: 5,
        on_death: DeathCallback::Player,
    });

    // create npc
    // let npc = Object::new(SCREEN_WIDTH / 2 -5, SCREEN_HEIGHT / 2, '&', YELLOW);

    // list of objects
    let mut objects = vec![player];

    // generate map
    let mut game = Game {
        map: make_map(&mut objects),
        messages: Messages::new(),
    };

    game.messages.add(
        "Welcome student! Prepare to perish in the Neoliberal Corporatocracy.",
        BLUE,
    );

    // populate fov map acc generated map
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !game.map[x as usize][y as usize].block_sight,
                !game.map[x as usize][y as usize].blocked,
            );
        }
    }

    // force FOV to recompute first time thru game loop
    let mut previous_player_position = (-1, -1);

    while !tcod.root.window_closed() {
        // clear screen of previous frame
        tcod.con.clear();

        // renders screen
        /* render_all(&mut tcod, &game, &objects);*/
        let fov_recompute = previous_player_position != (objects[PLAYER].x, objects[PLAYER].y);
        render_all(&mut tcod, &mut game, &objects, fov_recompute);

        // draws everything at once
        tcod.root.flush();

        // waits for input
        tcod.root.wait_for_keypress(true);

        for object in &objects {
               object.draw(&mut tcod.con);
        }

        
        
        // handle keys n exit game

        previous_player_position = objects[PLAYER].pos();
        let player_action = handle_keys(&mut tcod, &mut game, &mut objects);

        // let monsters take turn
        if objects[PLAYER].alive && player_action != PlayerAction::DidntTakeTurn {
            for id in 0..objects.len() {
                if objects[id].ai.is_some() {
                    ai_take_turn(id, &tcod, &mut game, &mut objects);
                }
            }
        }

        if player_action == PlayerAction::Exit {
            break;
        }
    }
}

