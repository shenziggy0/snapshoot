import React from 'react';
import { Toaster as HotToaster } from 'react-hot-toast';

const options = {
    style: { background: 'var(--snap-panel)', color: 'var(--snap-text)', borderRadius: '18px', boxShadow: 'var(--snap-shadow)' },
    success: { iconTheme: { primary: 'var(--snap-success)', secondary: 'var(--snap-panel)' } },
    error: { iconTheme: { primary: 'var(--snap-danger)', secondary: 'var(--snap-panel)' } },
    loading: { iconTheme: { primary: 'var(--snap-accent)', secondary: 'var(--snap-line)' } },
};

export default function Toaster(props) {
    return <HotToaster toastOptions={options} {...props} />;
}
