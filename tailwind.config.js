/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        accent: "var(--accent)",
      },
      backdropBlur: {
        glass: "40px",
      },
    },
  },
  plugins: [],
};
