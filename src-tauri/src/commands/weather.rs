use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct OpenMeteoResponse {
    current_weather: CurrentWeather,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temperature: f64,
    windspeed: f64,
    weathercode: u32,
}

#[derive(Debug, Serialize)]
pub struct WeatherResponse {
    pub temperature: f64,
    pub temperature_unit: String,
    pub windspeed: f64,
    pub windspeed_unit: String,
    pub condition: String,
}

#[derive(Debug, Deserialize)]
struct GeocodeResponse {
    results: Option<Vec<GeocodeResult>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GeocodeResult {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country: Option<String>,
}

fn weather_code_to_condition(code: u32) -> &'static str {
    match code {
        0 => "Clear sky",
        1 | 2 | 3 => "Partly cloudy",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 => "Freezing drizzle",
        61 | 63 | 65 => "Rain",
        66 | 67 => "Freezing rain",
        71 | 73 | 75 | 77 => "Snow",
        80 | 81 | 82 => "Rain showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorm",
        96 | 99 => "Thunderstorm with hail",
        _ => "Unknown",
    }
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn get_weather(lat: f64, lon: f64, fahrenheit: bool) -> Result<WeatherResponse, String> {
    let temperature_unit = if fahrenheit { "fahrenheit" } else { "celsius" };
    let windspeed_unit = if fahrenheit { "mph" } else { "kmh" };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current_weather=true&temperature_unit={temperature_unit}&windspeed_unit={windspeed_unit}"
    );
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to reach Open-Meteo: {e}"))?
        .json::<OpenMeteoResponse>()
        .await
        .map_err(|e| format!("Failed to parse Open-Meteo response: {e}"))?;

    Ok(WeatherResponse {
        temperature: resp.current_weather.temperature,
        temperature_unit: if fahrenheit { "°F" } else { "°C" }.to_string(),
        windspeed: resp.current_weather.windspeed,
        windspeed_unit: windspeed_unit.to_string(),
        condition: weather_code_to_condition(resp.current_weather.weathercode).to_string(),
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub async fn geocode_city(name: String) -> Result<GeocodeResult, String> {
    let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={name}&count=1");
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to reach geocoding service: {e}"))?
        .json::<GeocodeResponse>()
        .await
        .map_err(|e| format!("Failed to parse geocoding response: {e}"))?;

    resp.results
        .and_then(|mut r| {
            if r.is_empty() {
                None
            } else {
                Some(r.remove(0))
            }
        })
        .ok_or_else(|| format!("No location found matching '{name}'"))
}
