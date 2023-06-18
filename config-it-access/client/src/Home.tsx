import React, { useEffect } from 'react';
import { Store } from 'react-notifications-component';

export function Home() {
	useEffect(() => {
		Store.addNotification({
			title: "Hello!",
			message: "Welcome!",
			type: "info",
			container: "bottom-right",
			dismiss: { duration: 2000 }
		});

		console.log("pewpew");
	}, []);

	// TODO: 
	return <>
		<div className="flex flex-col items-center justify-center h-screen">
			<h1 className="text-6xl font-bold">Config-it-Access</h1>
			<h2 className="text-2xl font-bold">Configure Everything, Manage Anywhere</h2>
		</div>
	</>;

}
