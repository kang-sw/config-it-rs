import React from 'react';

export function Button(props: {
	children: React.ReactNode,
	color?: string,
	className?: string
	onClick?: () => void
}) {
	const color = props.color ?? "blue";

	return <button
		className={
			`px-4 py-2 transition-colors
			bg-${color}-500 hover:bg-${color}-600 hover:clicked:bg-${color}-100 
			active:bg-${color}-800 focus:outline-none focus:ring focus:ring-${color}-300
			text-white rounded-md ${props.className}`
		}
		onClick={props.onClick}>
		{props.children}
	</button>
}
