// Iconic Iron Man movie scenes/trailers Hacks can launch on command. Each maps
// spoken/typed aliases to a YouTube SEARCH query (search URLs never rot the way
// a specific video link does). Add a scene by appending here.
export interface IronManScene {
  id: string;
  title: string;
  aliases: string[];
  query: string;
}

export const IRON_MAN_SCENES: IronManScene[] = [
  { id: "mark1", title: "Mark I escape (Iron Man, 2008)", aliases: ["mark i escape", "mark 1 escape", "mark one escape", "cave escape", "first suit escape"], query: "Iron Man 2008 Mark I escape scene" },
  { id: "mark2flight", title: "Mark II flight test", aliases: ["mark ii flight", "mark 2 flight", "flight test", "first flight"], query: "Iron Man 2008 Mark II flight test scene" },
  { id: "iamironman", title: "\"I am Iron Man\" (2008)", aliases: ["i am iron man", "press conference", "reveal"], query: "Iron Man 2008 I am Iron Man ending scene" },
  { id: "suitup", title: "Mark III suit-up", aliases: ["suit up", "mark iii suit up", "mark 3 suit up", "gantry"], query: "Iron Man 2008 Mark III suit up gantry scene" },
  { id: "monaco", title: "Monaco whiplash attack (Iron Man 2)", aliases: ["monaco", "whiplash", "racetrack", "grand prix"], query: "Iron Man 2 Monaco Whiplash attack scene" },
  { id: "birthday", title: "Drunk birthday party fight (Iron Man 2)", aliases: ["birthday party", "drunk suit", "war machine fight", "rhodey fight"], query: "Iron Man 2 birthday party suit fight scene" },
  { id: "nyc", title: "Battle of New York (The Avengers)", aliases: ["battle of new york", "avengers battle", "nuke", "wormhole", "nuclear missile"], query: "The Avengers 2012 Battle of New York Iron Man nuke scene" },
  { id: "barrel", title: "Barrel of Monkeys rescue (Iron Man 3)", aliases: ["barrel of monkeys", "air force one", "falling people", "skydive rescue"], query: "Iron Man 3 Barrel of Monkeys Air Force One rescue scene" },
  { id: "housparty", title: "House Party Protocol (Iron Man 3)", aliases: ["house party protocol", "all the suits", "iron legion", "final battle iron man 3"], query: "Iron Man 3 House Party Protocol all suits final battle" },
  { id: "hulkbuster", title: "Hulkbuster vs Hulk (Age of Ultron)", aliases: ["hulkbuster", "veronica", "hulk fight", "hulkbuster fight"], query: "Avengers Age of Ultron Hulkbuster vs Hulk fight scene" },
  { id: "airport", title: "Airport battle (Civil War)", aliases: ["airport battle", "civil war fight", "team iron man", "spider man airport"], query: "Captain America Civil War airport battle scene" },
  { id: "titan", title: "Iron Man vs Thanos on Titan (Infinity War)", aliases: ["vs thanos", "titan fight", "thanos fight", "nano gauntlet", "stabbed"], query: "Avengers Infinity War Iron Man vs Thanos Titan fight scene" },
  { id: "portals", title: "Portals / final battle (Endgame)", aliases: ["portals", "final battle", "endgame battle", "assemble", "avengers assemble"], query: "Avengers Endgame portals assemble final battle scene" },
  { id: "snap", title: "\"I am Iron Man\" snap (Endgame)", aliases: ["endgame snap", "i love you 3000", "iron man snap", "tony dies", "final snap"], query: "Avengers Endgame Iron Man snap I am Iron Man scene" },
  { id: "trailer08", title: "Iron Man (2008) trailer", aliases: ["iron man trailer", "2008 trailer", "first trailer"], query: "Iron Man 2008 official trailer" },
];

export function findScene(text: string): IronManScene | undefined {
  const lc = text.toLowerCase();
  return IRON_MAN_SCENES.find((s) => s.aliases.some((a) => lc.includes(a)));
}

export function sceneUrl(scene: IronManScene): string {
  return `https://www.youtube.com/results?search_query=${encodeURIComponent(scene.query)}`;
}
