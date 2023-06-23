import { useEffect } from 'react';
import { Store } from 'react-notifications-component';
import { Button, Spinner } from './Widgets';

export function Dashboard() {
	// TODO: System overview page
	// - Management server status: CPU/memory/storage/network usage, etc ...
	// - Number of online/total sessions
	// - Number of online/total users

	async function onClick() {
		const res = await fetch('/api/sess/logout', { method: "POST" });
		const text = await res.text();
		console.log(text);
	}

	return <>
		<div className="flex flex-col items-center justify-center h-screen">
			<Button onClick={onClick} theme="info">Click-Me!</Button>
		</div>
	</>
}

export function About() {
	useEffect(() => {
		Store.addNotification({
			title: "Hello!",
			message: <div>Click Me!</div>,
			type: "success",
			container: "bottom-right",
			dismiss: { duration: 1000 },
		});
	}, []);

	return <>
		<div className="flex flex-col items-center justify-center h-full bg-slate-800">
			<h1 className="text-6xl font-bold text-white"><PrettyConfigItAccessText /></h1>
			<h2 className="text-2xl mt-3 font-bold font-mono text-gray-400">configure everything, control anywhere</h2>

			<div className='flex flex-row text-white'>
				<Spinner style='ring' className='text-2xl text-green-400' />
				<div className='self-center'> This is text</div>
				<Spinner style='flower' className='text-2xl' />
				<Spinner style='ping' className='self-center' />
				<Spinner style='arrow' className='text-2xl' />

			</div>
		</div >
	</>
}

export function PrettyConfigItAccessText() {
	const dash = <span className='text-gray-400'>-</span>
	return <div className='flex'>
		config{dash}it{dash}
		<div className='text-green-400 transition-all hover:-translate-y-1/4'>
			[access]
		</div>
	</div>
}

export function RepoIcon(props: { className?: string }) {
	return <a href='https://github.com/kang-sw/config-it-rs'>
		<svg height="32" aria-hidden="true"
			viewBox="0 0 16 16" version="1.1" width="32"
			data-view-component="true" className={props.className}>
			<path d="M8 0c4.42 0 8 3.58 8 8a8.013 8.013 0 0 1-5.45 7.59c-.4.08-.55-.17-.55-.38 0-.27.01-1.13.01-2.2 0-.75-.25-1.23-.54-1.48 1.78-.2 3.65-.88 3.65-3.95 0-.88-.31-1.59-.82-2.15.08-.2.36-1.02-.08-2.12 0 0-.67-.22-2.2.82-.64-.18-1.32-.27-2-.27-.68 0-1.36.09-2 .27-1.53-1.03-2.2-.82-2.2-.82-.44 1.1-.16 1.92-.08 2.12-.51.56-.82 1.28-.82 2.15 0 3.06 1.86 3.75 3.64 3.95-.23.2-.44.55-.51 1.07-.46.21-1.61.55-2.33-.66-.15-.24-.6-.83-1.23-.82-.67.01-.27.38.01.53.34.19.73.9.82 1.13.16.45.68 1.31 2.69.94 0 .67.01 1.3.01 1.49 0 .21-.15.45-.55.38A7.995 7.995 0 0 1 0 8c0-4.42 3.58-8 8-8Z" />
		</svg>
	</a>
}
