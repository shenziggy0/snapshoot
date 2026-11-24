const { nextui } = require('@nextui-org/react');

const blue = {
    50: '#f0f6ff', 100: '#e4efff', 200: '#cbdfff', 300: '#9ec9ff', 400: '#65a8ff',
    500: '#0865cf', 600: '#0658b4', 700: '#12468b', 800: '#193561', 900: '#14223d',
};
const gray = {
    50: '#f8f9fb', 100: '#eff0f4', 200: '#e0e2e9', 300: '#c9cbd5', 400: '#9295a3',
    500: '#646774', 600: '#4a4d59', 700: '#343641', 800: '#24252e', 900: '#16171d',
};

function semanticTheme(dark) {
    const shades = (palette) => dark
        ? Object.fromEntries(Object.keys(palette).map((key, i) => [key, Object.values(palette).reverse()[i]]))
        : palette;
    return {
        colors: {
            background: dark ? '#18191f' : '#f5f5f9',
            foreground: dark ? '#f6f6fb' : '#20212a',
            content1: dark ? '#26272e' : '#ffffff',
            content2: dark ? '#33343d' : '#eceef4',
            content3: dark ? '#41434d' : '#e1e4eb',
            content4: dark ? '#51535f' : '#d5d8e2',
            divider: dark ? '#444650' : '#dbdde5',
            focus: dark ? '#83bdff' : blue[500],
            default: { ...shades(gray), DEFAULT: dark ? '#3b3d48' : gray[200], foreground: dark ? '#fff' : gray[900] },
            primary: { ...shades(blue), DEFAULT: dark ? '#87beff' : blue[500], foreground: dark ? '#102642' : '#fff' },
            secondary: { DEFAULT: dark ? '#c3b8ff' : '#6453ab', foreground: dark ? '#231947' : '#fff' },
            success: { DEFAULT: dark ? '#76d69c' : '#237c49', foreground: dark ? '#103520' : '#fff' },
            warning: { DEFAULT: dark ? '#ffcb77' : '#92600c', foreground: dark ? '#372500' : '#fff' },
            danger: { DEFAULT: dark ? '#ff9096' : '#b8293d', foreground: dark ? '#410c12' : '#fff' },
        },
    };
}

module.exports = {
    content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}', './node_modules/@nextui-org/theme/dist/**/*.{js,ts,jsx,tsx}'],
    theme: { extend: { colors: { snap: blue, ink: gray } } },
    darkMode: 'class',
    plugins: [nextui({
        layout: { radius: { small: '10px', medium: '15px', large: '20px' } },
        themes: { light: semanticTheme(false), dark: semanticTheme(true) },
    })],
};
