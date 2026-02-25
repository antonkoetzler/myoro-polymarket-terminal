//! Sports data: scraper (FBRef/FixtureDownload), OpenFootball, FBRef xG, live scores, Kalshi.

pub mod kalshi;
pub mod live_scores;
pub mod openfootball;
mod scraper;
pub mod xg;

#[allow(unused_imports)]
pub use kalshi::KalshiClient;
#[allow(unused_imports)]
pub use live_scores::{LiveMatchState, LiveScoresClient, MatchStatus};
#[allow(unused_imports)]
pub use openfootball::OpenFootballClient;
#[allow(unused_imports)]
pub use scraper::{Fixture, SportsScraper};
#[allow(unused_imports)]
pub use xg::{TeamXgStats, XgScraper};
