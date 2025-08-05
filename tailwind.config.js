/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}"
  ],
  theme: {
    extend: {
      fontFamily: {
        display: ["Montserrat", "ui-sans-serif", "system-ui"],
        body: ["Inter", "ui-sans-serif", "system-ui"],
      },
      colors: {
        brand: {
          DEFAULT: "#1db954", // Spotify green for music vibe
          dark: "#191414",
          light: "#1ed760",
        },
        surface: {
          DEFAULT: "#23272a",
          light: "#2c2f33",
          dark: "#18191a"
        },
        accent: {
          DEFAULT: "#ff5e3a",
          light: "#ffb199",
        }
      },
      boxShadow: {
        card: "0 4px 32px 0 rgba(30,185,84,0.12)",
      }
    },
  },
  plugins: [],
}

