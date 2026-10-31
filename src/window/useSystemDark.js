import { useEffect, useState } from 'react';

export function useSystemDark() {
    const query = '(prefers-color-scheme: dark)';
    const [dark, setDark] = useState(() => window.matchMedia(query).matches);

    useEffect(() => {
        const media = window.matchMedia(query);
        const onChange = () => setDark(media.matches);
        media.addEventListener('change', onChange);
        return () => media.removeEventListener('change', onChange);
    }, []);

    return dark;
}
