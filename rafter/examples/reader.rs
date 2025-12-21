//! Reader Example
//!
//! A simple scrollable text reader to test the ScrollArea widget.

use std::fs::File;

use log::LevelFilter;
use rafter::prelude::*;
use simplelog::{Config, WriteLogger};

const LONG_TEXT: &str = r#"
The Wandering Clockmaker

In the village of Thornwick, where the cobblestones grew moss in peculiar patterns, there lived a clockmaker who had forgotten how to measure time. His name was Elias Pendulum, though the children called him "Tick-Tock" for the way he muttered rhythms under his breath.

Every morning, Elias would open his shop at what he believed was dawn, though it was often noon or sometimes midnight. The townsfolk had learned to check the position of the sun before trusting his hours. His clocks, however, were magnificent contraptions of brass and crystal that told not the time, but the weather three days hence, the mood of the nearest cat, and occasionally, the winning numbers of lotteries that hadn't been invented yet.

One autumn evening, a traveler arrived seeking a clock that could remind her of home. She had wandered so far and so long that she'd forgotten which direction home lay. Elias scratched his silver beard and retreated to his workshop.

For seven days (or perhaps seven hours, nobody could tell), the sounds of hammering and the smell of burning sage drifted from behind the curtain. When Elias emerged, he carried a small pocket watch made of river stones and spider silk.

"This won't tell you where home is," he explained, pressing it into her palm. "But it will tick louder whenever you're walking in the right direction. And when you arrive, it will stop completely."

The traveler thanked him and departed. Years later, a letter arrived at the shop. Inside was the watch, silent and still, along with a single pressed flower from a garden the traveler had planted in a place she now called home.

Elias smiled and hung the watch in his window, where it joined dozens of others - each one a story, each one stopped at the perfect moment.

The Quantum Bakery

Madame Croissant (her real name was Margaret, but she'd legally changed it after winning the Great Bake-Off of '87) ran the only bakery in town where the pastries existed in multiple states simultaneously.

Her famous uncertainty rolls were both burnt and perfectly golden until you opened the bag. Her probability pies contained every possible filling until you took the first bite. And her observation scones would only rise if nobody was watching the oven.

Scientists came from distant universities to study her methods. She would shoo them away with a flour-dusted rolling pin, insisting that the magic worked precisely because she didn't understand it.

"Understanding ruins everything," she would say, sliding another tray of superposition sourdough into her antique oven. "The bread knows what it wants to be. I just give it options."

Her most famous creation was the paradox pretzel - a twisted loop that was somehow always fresh, even if left out for weeks. Physicists theorized it existed outside normal spacetime. Madame Croissant said it was just good yeast.

The Memory Librarian

In the basement of the old library, past the section on Forgotten Languages and the alcove of Unwritten Books, there was a door that only appeared on rainy Thursdays. Behind it sat the Memory Librarian, a woman of indeterminate age who collected not books, but recollections.

Visitors could deposit memories they no longer wished to carry - the sting of an old betrayal, the weight of a regret, the sharp edge of a loss too heavy to bear. She would carefully catalog each one, pressing them like flowers between the pages of silver-leafed journals.

Some came to borrow memories instead: the joy of a first kiss, the wonder of seeing the ocean, the comfort of a grandmother's kitchen. These she would lend for exactly three days, no more, no less.

"Memories aren't meant to be owned," she would explain. "Only experienced."

Her most treasured collection was the shelf of anonymous kindnesses - small moments of grace that the givers had forgotten but the recipients had cherished. These, she never lent out. They were too precious, too easily worn thin by handling.

On quiet evenings, she would sometimes open a random journal and let a stranger's sunset wash over her, or feel the phantom warmth of a hug given decades ago. In this way, nothing was ever truly lost.

The Garden of Possible Tomorrows

At the edge of town, where the pavement crumbled into wildflower meadows, an elderly gardener tended plots that grew not vegetables, but futures. Each seed contained a different tomorrow, and with careful cultivation, one could harvest the destiny of their choosing.

Some plots grew careers, their leaves shaped like briefcases or stethoscopes or paintbrushes. Others sprouted relationships, vines that intertwined in complex patterns. A few rare specimens produced adventures, their fruits bursting with the scent of salt air or mountain snow.

The gardener never planted anything for himself. He'd tried once, in his youth, and grown a future so beautiful it had paralyzed him with fear of not deserving it. That plot still sat empty, overgrown with ordinary weeds - the only truly wild corner of his orderly domain.

"The future doesn't like to be forced," he would tell visitors. "You can only create the conditions for it to grow."

And every evening, as the sun painted the sky in colors no painter could capture, he would walk among his strange crops and wonder which tomorrows would take root, which would wither, and which would bloom into something none of them could have imagined.

[End of excerpts from "Tales from the Improbable Quarter" - a collection that exists only in this moment]
"#;

#[app]
struct Reader {
    scroll: ScrollArea,
}

#[app_impl]
impl Reader {
    #[keybinds]
    fn keys() -> Keybinds {
        keybinds! {
            "q" => quit,
        }
    }

    #[handler]
    async fn quit(&self, cx: &AppContext) {
        cx.exit();
    }

    fn page(&self) -> Node {
        page! {
            row(padding: 2, bg: background) {
                column(padding: 4) {}
                column(gap: 1) {
                    text(bold, fg: primary) { "Reader - Press 'q' to quit, scroll with mouse wheel" }
                    column(border: rounded, padding: 2) {
                        scroll_area(bind: self.scroll, direction: vertical) {
                            text(fg: text) { LONG_TEXT }
                        }
                    }
                }
                column(padding: 4) {}
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize file logging
    if let Ok(log_file) = File::create("reader.log") {
        let _ = WriteLogger::init(LevelFilter::Debug, Config::default(), log_file);
    }

    if let Err(e) = rafter::Runtime::new().start_with::<Reader>().await {
        eprintln!("Error: {}", e);
    }
}
