/** @type {import('tailwindcss').Config} */
module.exports = {
    content: [
        'src/**/*.{html,rs}',
        'public/**/*.{html,rs}',
    ],
    plugins: [
        require('@tailwindcss/forms'),
        require('@tailwindcss/typography'),
    ],
    theme: {},
};
