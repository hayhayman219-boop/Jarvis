// Mainline Pokémon games (core RPGs, enhanced versions, remakes, and the
// Legends line). To add a newly released title, just append it here — the
// checklist picks it up automatically, grouped by generation.
export interface PokemonGame {
  id: string;
  title: string;
  gen: number;
  year: number;
  platform: string;
}

export const POKEMON_GAMES: PokemonGame[] = [
  // Gen 1
  { id: "red", title: "Pokémon Red", gen: 1, year: 1996, platform: "Game Boy" },
  { id: "blue", title: "Pokémon Blue", gen: 1, year: 1996, platform: "Game Boy" },
  { id: "yellow", title: "Pokémon Yellow", gen: 1, year: 1998, platform: "Game Boy" },
  // Gen 2
  { id: "gold", title: "Pokémon Gold", gen: 2, year: 1999, platform: "Game Boy Color" },
  { id: "silver", title: "Pokémon Silver", gen: 2, year: 1999, platform: "Game Boy Color" },
  { id: "crystal", title: "Pokémon Crystal", gen: 2, year: 2000, platform: "Game Boy Color" },
  // Gen 3
  { id: "ruby", title: "Pokémon Ruby", gen: 3, year: 2002, platform: "Game Boy Advance" },
  { id: "sapphire", title: "Pokémon Sapphire", gen: 3, year: 2002, platform: "Game Boy Advance" },
  { id: "emerald", title: "Pokémon Emerald", gen: 3, year: 2004, platform: "Game Boy Advance" },
  { id: "firered", title: "Pokémon FireRed", gen: 3, year: 2004, platform: "Game Boy Advance" },
  { id: "leafgreen", title: "Pokémon LeafGreen", gen: 3, year: 2004, platform: "Game Boy Advance" },
  // Gen 4
  { id: "diamond", title: "Pokémon Diamond", gen: 4, year: 2006, platform: "Nintendo DS" },
  { id: "pearl", title: "Pokémon Pearl", gen: 4, year: 2006, platform: "Nintendo DS" },
  { id: "platinum", title: "Pokémon Platinum", gen: 4, year: 2008, platform: "Nintendo DS" },
  { id: "heartgold", title: "Pokémon HeartGold", gen: 4, year: 2009, platform: "Nintendo DS" },
  { id: "soulsilver", title: "Pokémon SoulSilver", gen: 4, year: 2009, platform: "Nintendo DS" },
  // Gen 5
  { id: "black", title: "Pokémon Black", gen: 5, year: 2010, platform: "Nintendo DS" },
  { id: "white", title: "Pokémon White", gen: 5, year: 2010, platform: "Nintendo DS" },
  { id: "black2", title: "Pokémon Black 2", gen: 5, year: 2012, platform: "Nintendo DS" },
  { id: "white2", title: "Pokémon White 2", gen: 5, year: 2012, platform: "Nintendo DS" },
  // Gen 6
  { id: "x", title: "Pokémon X", gen: 6, year: 2013, platform: "Nintendo 3DS" },
  { id: "y", title: "Pokémon Y", gen: 6, year: 2013, platform: "Nintendo 3DS" },
  { id: "omegaruby", title: "Pokémon Omega Ruby", gen: 6, year: 2014, platform: "Nintendo 3DS" },
  { id: "alphasapphire", title: "Pokémon Alpha Sapphire", gen: 6, year: 2014, platform: "Nintendo 3DS" },
  // Gen 7
  { id: "sun", title: "Pokémon Sun", gen: 7, year: 2016, platform: "Nintendo 3DS" },
  { id: "moon", title: "Pokémon Moon", gen: 7, year: 2016, platform: "Nintendo 3DS" },
  { id: "ultrasun", title: "Pokémon Ultra Sun", gen: 7, year: 2017, platform: "Nintendo 3DS" },
  { id: "ultramoon", title: "Pokémon Ultra Moon", gen: 7, year: 2017, platform: "Nintendo 3DS" },
  { id: "letsgopikachu", title: "Pokémon: Let's Go, Pikachu!", gen: 7, year: 2018, platform: "Switch" },
  { id: "letsgoeevee", title: "Pokémon: Let's Go, Eevee!", gen: 7, year: 2018, platform: "Switch" },
  // Gen 8
  { id: "sword", title: "Pokémon Sword", gen: 8, year: 2019, platform: "Switch" },
  { id: "shield", title: "Pokémon Shield", gen: 8, year: 2019, platform: "Switch" },
  { id: "brilliantdiamond", title: "Pokémon Brilliant Diamond", gen: 8, year: 2021, platform: "Switch" },
  { id: "shiningpearl", title: "Pokémon Shining Pearl", gen: 8, year: 2021, platform: "Switch" },
  { id: "legendsarceus", title: "Pokémon Legends: Arceus", gen: 8, year: 2022, platform: "Switch" },
  // Gen 9
  { id: "scarlet", title: "Pokémon Scarlet", gen: 9, year: 2022, platform: "Switch" },
  { id: "violet", title: "Pokémon Violet", gen: 9, year: 2022, platform: "Switch" },
  { id: "legendsza", title: "Pokémon Legends: Z-A", gen: 9, year: 2025, platform: "Switch / Switch 2" },
];
