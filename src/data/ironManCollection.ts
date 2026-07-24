// Key Iron Man comics and MCU film appearances, for the collection tracker.
// Append to grow it — the tracker groups by `category` automatically.
export interface CollectionItem {
  id: string;
  title: string;
  category: "Key Comics" | "Films";
  year: number;
  note: string;
}

export const IRON_MAN_COLLECTION: CollectionItem[] = [
  // Key comics
  { id: "tos39", title: "Tales of Suspense #39", category: "Key Comics", year: 1963, note: "1st appearance of Iron Man" },
  { id: "tos48", title: "Tales of Suspense #48", category: "Key Comics", year: 1964, note: "1st red-and-gold armor" },
  { id: "im1v1", title: "Iron Man #1 (vol. 1)", category: "Key Comics", year: 1968, note: "1st solo series" },
  { id: "im55", title: "Iron Man #55", category: "Key Comics", year: 1973, note: "1st Thanos & Drax" },
  { id: "im128", title: "Iron Man #128", category: "Key Comics", year: 1979, note: "\"Demon in a Bottle\" cover" },
  { id: "im225", title: "Iron Man #225", category: "Key Comics", year: 1987, note: "\"Armor Wars\" begins" },
  { id: "im282", title: "Iron Man #282", category: "Key Comics", year: 1992, note: "1st full War Machine armor" },
  { id: "extremis1", title: "Iron Man: Extremis #1", category: "Key Comics", year: 2005, note: "Warren Ellis; MCU basis" },
  { id: "invincible1", title: "Invincible Iron Man #1", category: "Key Comics", year: 2008, note: "Fraction/Larroca run" },
  { id: "superior1", title: "Superior Iron Man #1", category: "Key Comics", year: 2014, note: "Inverted Tony" },
  { id: "ironheart1", title: "Invincible Iron Man #1 (Ironheart)", category: "Key Comics", year: 2016, note: "Riri Williams era" },

  // Films
  { id: "im2008", title: "Iron Man", category: "Films", year: 2008, note: "The one that started the MCU" },
  { id: "im2", title: "Iron Man 2", category: "Films", year: 2010, note: "Whiplash, War Machine debut" },
  { id: "avengers", title: "The Avengers", category: "Films", year: 2012, note: "Battle of New York" },
  { id: "im3", title: "Iron Man 3", category: "Films", year: 2013, note: "The Mandarin, House Party" },
  { id: "aou", title: "Avengers: Age of Ultron", category: "Films", year: 2015, note: "Hulkbuster, Ultron" },
  { id: "civilwar", title: "Captain America: Civil War", category: "Films", year: 2016, note: "Airport battle" },
  { id: "homecoming", title: "Spider-Man: Homecoming", category: "Films", year: 2017, note: "Mentor Tony" },
  { id: "infinitywar", title: "Avengers: Infinity War", category: "Films", year: 2018, note: "Titan, nano-armor" },
  { id: "endgame", title: "Avengers: Endgame", category: "Films", year: 2019, note: "\"I am Iron Man.\"" },
];
