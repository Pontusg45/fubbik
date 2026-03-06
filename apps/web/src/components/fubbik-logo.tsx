export default function FubbikLogo({ className }: { className?: string }) {
    return (
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="95 55 190 190" className={className}>
            <line x1="200" y1="180" x2="200" y2="85" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            <line x1="200" y1="180" x2="270" y2="135" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            <line x1="200" y1="180" x2="265" y2="225" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            <line x1="200" y1="180" x2="135" y2="225" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            <line x1="200" y1="180" x2="130" y2="135" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            <circle cx="200" cy="85" r="20" fill="currentColor" stroke="currentColor" strokeWidth="2.5" />
            <circle cx="270" cy="140" r="15" fill="currentColor" stroke="currentColor" strokeWidth="2.5" />
            <circle cx="265" cy="225" r="15" fill="currentColor" stroke="currentColor" strokeWidth="2.5" />
            <circle cx="135" cy="225" r="15" fill="currentColor" stroke="currentColor" strokeWidth="2.5" />
            <circle cx="130" cy="140" r="15" fill="currentColor" stroke="currentColor" strokeWidth="2.5" />
            <circle cx="200" cy="180" r="25" fill="currentColor" stroke="currentColor" strokeWidth="2.5" />
        </svg>
    );
}
