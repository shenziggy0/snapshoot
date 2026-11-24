import React from 'react';
import './tools.css';

export default function ToolButton({ title, active, disabled, onClick, primary, children }) {
    return (
        <button
            type='button'
            title={title}
            aria-label={title}
            aria-pressed={active}
            disabled={disabled}
            onMouseDown={(event) => event.stopPropagation()}
            onClick={onClick}
            className={primary ? 'snap-tool snap-tool-primary' : 'snap-tool'}
        >
            {children}
        </button>
    );
}
