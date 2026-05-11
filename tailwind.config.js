/** @type {import('tailwindcss').Config} */
export default {
  content: ["./src/**/*.{html,js,svelte,ts}"],
  theme: {
    extend: {
      fontFamily: {
        arabic: ["SF Arabic", "IBM Plex Sans Arabic", "system-ui"]
      }
    }
  },
  plugins: []
};
