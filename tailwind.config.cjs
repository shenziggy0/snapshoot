const { nextui } = require('@nextui-org/react');
const plugin = require('tailwindcss/plugin');

const blue = {
    50: '#f3f7ff', 100: '#e8f0fe', 200: '#c8dafe', 300: '#a5c4fc', 400: '#88b3ff',
    500: '#2f6feb', 600: '#2560d6', 700: '#204eab', 800: '#203451', 900: '#192c47',
};
const gray = {
    50: '#fbfbfb', 100: '#f3f3f4', 200: '#ebebed', 300: '#d6d6da', 400: '#8e8e93',
    500: '#65656b', 600: '#505056', 700: '#3c3c43', 800: '#29292d', 900: '#1d1d1f',
};

// Both the window CSS and the component library use the same surface and action colors.
const light = {
    paper: '#fbfbfb', panel: '#ffffff', well: '#f3f3f4', hover: '#ebebed',
    text: '#1d1d1f', secondary: '#3c3c43', muted: '#65656b', faint: '#c2c2c7',
    line: '#eaeaea', 'line-strong': '#d6d6da', accent: '#2f6feb', link: '#2560d6',
    'accent-hover': '#2560d6', 'accent-soft': '#e8f0fe', focus: '#3b82f6',
};
const dark = {
    paper: '#101011', panel: '#141416', well: '#222224', hover: '#29292d',
    text: '#f0efed', secondary: '#cfcecc', muted: '#a3a3a8', faint: '#74747c',
    line: '#2c2c2f', 'line-strong': '#414145', accent: '#3672d6', link: '#88b3ff',
    'accent-hover': '#326ac6', 'accent-soft': '#203451', focus: '#88b3ff',
};

function semanticTheme(tokens, isDark) {
    const shades = (palette) => isDark
        ? Object.fromEntries(Object.keys(palette).map((key, i) => [key, Object.values(palette).reverse()[i]]))
        : palette;
    return {
        colors: {
            background: tokens.paper, foreground: tokens.text, content1: tokens.panel, content2: tokens.well,
            content3: tokens.hover, content4: tokens['line-strong'], divider: tokens.line, focus: tokens.focus,
            default: { ...shades(gray), DEFAULT: tokens.hover, foreground: tokens.text },
            primary: { ...shades(blue), DEFAULT: tokens.accent, foreground: '#ffffff' },
            secondary: { DEFAULT: isDark ? '#c3b8ff' : '#6453ab', foreground: isDark ? '#231947' : '#fff' },
            success: { DEFAULT: isDark ? '#76d69c' : '#237c49', foreground: isDark ? '#103520' : '#fff' },
            warning: { DEFAULT: isDark ? '#ffcb77' : '#92600c', foreground: isDark ? '#372500' : '#fff' },
            danger: { DEFAULT: isDark ? '#ff9096' : '#b8293d', foreground: isDark ? '#410c12' : '#fff' },
        },
    };
}

const variables = (tokens) => Object.fromEntries(Object.entries(tokens).map(([key, value]) => [`--snap-${key}`, value]));

module.exports = {
    content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}', './node_modules/@nextui-org/theme/dist/**/*.{js,ts,jsx,tsx}'],
    theme: { extend: { colors: { snap: blue, ink: gray } } },
    darkMode: 'class',
    plugins: [
        nextui({
            layout: { radius: { small: '8px', medium: '10px', large: '12px' } },
            themes: { light: semanticTheme(light, false), dark: semanticTheme(dark, true) },
        }),
        plugin(({ addBase }) => addBase({
            ':root': variables(light),
            '.dark': variables(dark),
            '.snap-glass, [data-slot="content"]:has([role="menu"], [role="listbox"])': variables(light),
        })),
    ],
};
