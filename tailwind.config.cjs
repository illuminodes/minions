/** @type {import('tailwindcss').Config} */
module.exports = {
    content: ['**/*.{html,rs}'],
    plugins: [
        require('@tailwindcss/forms'),
        require('@tailwindcss/typography'),
    ],
    theme: {},
};
